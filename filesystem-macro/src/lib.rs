use proc_macro::TokenStream;

use quote::quote;
use syn::{
    parse_macro_input,
    punctuated::Punctuated,
    token::{Comma, Semi},
    BareFnArg, Expr, Fields, GenericArgument, Ident, ItemImpl, ItemStruct, PathArguments,
    ReturnType, Stmt, Type, TypeBareFn, TypePtr,
};

fn is_ident(ty: &Type, ident: &str) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().unwrap().ident.to_string() == ident)
}

struct UnsafeFnConvert {
    new_inputs: Punctuated<BareFnArg, Comma>,
    converted_call: Punctuated<Expr, Comma>,
    conversion: Punctuated<Stmt, Semi>,
}

impl UnsafeFnConvert {
    fn sub_type(ty: Type) -> Type {
        syn::parse(
            match ty {
                Type::Ptr(TypePtr {
                    mutability, elem, ..
                }) => {
                    let sub = Self::sub_type(*elem);
                    quote!(& #mutability #sub)
                }
                ty if is_ident(&ty, "c_char") => quote!(u8),
                ty => quote!(#ty),
            }
            .into(),
        )
        .unwrap()
    }

    fn new(inputs: Punctuated<BareFnArg, Comma>) -> Self {
        let mut new_inputs = Punctuated::new();
        let mut converted_call = Punctuated::new();
        let mut conversions: Vec<Stmt> = vec![];

        let mut lookahead = inputs.clone().into_iter().skip(1);
        let mut inputs = inputs.into_iter();

        while let Some(arg) = inputs.next() {
            let next = lookahead.next();
            let sized = matches!(&next, Some(next) if is_ident(&next.ty, "size_t"));
            let size_ident = next.map(|n| n.name.unwrap().0);

            let ident = arg.name.unwrap().0;
            let new_ty: Type = match arg.ty {
                Type::Ptr(TypePtr {
                    mutability,
                    const_token,
                    elem,
                    ..
                }) if sized => {
                    let sub_ty = Self::sub_type(*elem);
                    let ty = syn::parse(quote!(&#mutability [#sub_ty]).into()).unwrap();

                    inputs.next();
                    let size_ident = size_ident.unwrap();
                    let slice_from: Ident = syn::parse(
                        if mutability.is_none() {
                            quote!(from_raw_parts)
                        } else {
                            quote!(from_raw_parts_mut)
                        }
                        .into(),
                    )
                    .unwrap();

                    conversions.push(
                        syn::parse(quote!(let #ident = std::slice::#slice_from (#ident as * #const_token #mutability #sub_ty, #size_ident as usize);).into()
                    ).expect("1"));
                    ty
                }

                Type::Ptr(TypePtr {
                    mutability: None,
                    elem,
                    ..
                }) if is_ident(&elem, "c_char") => {
                    let ty = syn::parse(quote!(&str).into()).unwrap();
                    conversions.push(
                        syn::parse(
                            quote!(let #ident = std::ffi::CStr::from_ptr(#ident).to_str().unwrap();)
                                .into(),
                        )
                        .expect("2"),
                    );
                    ty
                }

                Type::Ptr(TypePtr {
                    mutability, elem, ..
                }) => {
                    let ty = syn::parse(quote!(& #mutability #elem).into()).unwrap();

                    let ref_from: Ident = syn::parse(
                        if mutability.is_none() {
                            quote!(as_ref)
                        } else {
                            quote!(as_mut)
                        }
                        .into(),
                    )
                    .unwrap();

                    conversions
                        .push(syn::parse(quote!(let #ident = #ident . #ref_from ().unwrap();).into()).expect("3"));
    
                    ty
                }
                ty => ty,
            };

            new_inputs.push(syn::parse(quote!(#ident: #new_ty).into()).expect("new_inputs"));
            converted_call.push(syn::parse(quote!(#ident).into()).expect("converted_call"));
        }

        Self {
            new_inputs,
            converted_call,
            conversion: conversions.into_iter().collect(),
        }
    }
}

#[proc_macro_attribute]
pub fn fuse_operations(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut out = item.clone();
    let tokens = parse_macro_input!(item as ItemStruct);

    let fields = match tokens.fields {
        Fields::Named(fields) => fields.named,
        _ => unimplemented!(),
    };

    let mut raw_trait_fns = TokenStream::new();
    let mut trait_fns = TokenStream::new();

    for field in fields {
        let ty_path = match field.ty {
            Type::Path(path) => path,
            _ => continue,
        };

        let ty = ty_path.path.segments.last().unwrap();
        if ty.ident != "Option" {
            continue;
        }

        let args = match &ty.arguments {
            PathArguments::AngleBracketed(args) => args,
            _ => continue,
        };

        
        let TypeBareFn {
            unsafety,
            abi,
            inputs,
            variadic,
            output,
            ..
        } = match args.args.first().unwrap() {
            GenericArgument::Type(Type::BareFn(ty)) => ty,
            _ => continue,
        };
        
        if variadic.is_some()
            || !matches!(output, ReturnType::Type(_, ty)
                if is_ident(&ty, "c_int")
            )
        {
            continue;
        }

        
        let name = field.ident.unwrap();
        let UnsafeFnConvert {
            new_inputs,
            converted_call,
            conversion,
        } = UnsafeFnConvert::new(inputs.clone());

        let op_fn: TokenStream = quote! {
            fn #name (&mut self, #new_inputs) -> std::result::Result<(), i32> {
                std::result::Result::Ok(())
            }
        }
        .into();
        let raw_op_fn: TokenStream = quote! {
            #unsafety #abi fn #name (#inputs) #output {
                #conversion

                let out = ((*fuse_get_context()).private_data as *mut Self).as_mut().unwrap().#name (
                    #converted_call
                );

                match out {
                    Ok(()) => 0,
                    Err(e) => e,
                }
            }
        }
        .into();

        trait_fns.extend([op_fn]);
        raw_trait_fns.extend([raw_op_fn]);
    }

    let traits: TokenStream = format!(
        "pub trait FileSystem: Sized {{ {trait_fns} }} pub trait FileSystemRaw: FileSystem {{ {raw_trait_fns} }}"
    )
    .parse()
    .unwrap();

    out.extend([traits]);
    out
}

#[proc_macro_attribute]
pub fn fuse_main(attr: TokenStream, item: TokenStream) -> TokenStream {
    assert!(attr.is_empty(), "Expected no attributes");

    let mut out = item.clone();
    let tokens = parse_macro_input!(item as ItemImpl);

    let generics = tokens.generics;
    let ty = tokens.self_ty;

    let fuse_main_impl: TokenStream = quote! {
        impl #generics fuse_rs::FuseMain for #ty {
            fn run(fuse_args: &[&str]) -> Result<(), i32> {
                let mut operations = fuse_rs::__private::fuse_operations::default();

                todo!()
            }
        }
    }
    .into();

    out.extend([fuse_main_impl]);
    out
}
