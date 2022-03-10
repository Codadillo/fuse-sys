use std::collections::HashSet;

use proc_macro::TokenStream;

use quote::quote;
use proc_macro2::TokenStream as TokenStream2;
use syn::{
    parse::Parser,
    parse_macro_input,
    punctuated::Punctuated,
    token::{Comma, Semi},
    BareFnArg, Expr, Fields, GenericArgument, Ident, ItemStruct, PathArguments, ReturnType, Stmt,
    Type, TypeBareFn, TypePtr,
};

fn is_ident(ty: &Type, ident: &str) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().unwrap().ident.to_string() == ident)
}

struct UnsafeFnConvert {
    new_inputs: Punctuated<BareFnArg, Comma>,
    converted_call: Punctuated<Expr, Comma>,
    conversion: Punctuated<Stmt, Semi>,
    reexport_types: HashSet<String>,
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
        let mut reexport_types = HashSet::new();
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
                    ).unwrap());
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
                        .unwrap(),
                    );
                    ty
                }

                Type::Ptr(TypePtr {
                    mutability, elem, ..
                }) => {
                    let ty = syn::parse(quote!(Option<& #mutability #elem>).into()).unwrap();
                    if let Type::Path(path) = *elem {
                        if let Some(ident) = path.path.get_ident() {
                            reexport_types.insert(ident.to_string());
                        }
                    }

                    let ref_from: Ident = syn::parse(
                        if mutability.is_none() {
                            quote!(as_ref)
                        } else {
                            quote!(as_mut)
                        }
                        .into(),
                    )
                    .unwrap();

                    conversions.push(
                        syn::parse(quote!(let #ident = #ident . #ref_from ();).into()).unwrap(),
                    );

                    ty
                }

                Type::Path(path) => {
                    if let Some(ident) = path.path.get_ident() {
                        reexport_types.insert(ident.to_string());
                    }
                    Type::Path(path)
                }

                ty => ty,
            };

            new_inputs.push(syn::parse(quote!(#ident: #new_ty).into()).unwrap());
            converted_call.push(syn::parse(quote!(#ident).into()).unwrap());
        }

        Self {
            new_inputs,
            converted_call,
            reexport_types,
            conversion: conversions.into_iter().collect(),
        }
    }
}

#[proc_macro_attribute]
pub fn fuse_operations(attr: TokenStream, item: TokenStream) -> TokenStream {
    let auto_ok = Punctuated::<Ident, Comma>::parse_terminated
        .parse(attr)
        .map(|p| p.into_iter().collect::<Vec<_>>())
        .unwrap();

    let out: TokenStream2 = item.clone().into();
    let tokens = parse_macro_input!(item as ItemStruct);

    let fields = match tokens.fields {
        Fields::Named(fields) => fields.named,
        _ => unimplemented!(),
    };

    let mut raw_trait_fns = TokenStream2::new();
    let mut trait_fns = TokenStream2::new();
    let mut op_assignments: Vec<Stmt> = vec![];
    let mut all_reexport_types = HashSet::new();

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
            reexport_types,
            conversion,
        } = UnsafeFnConvert::new(inputs.clone());

        all_reexport_types.extend(reexport_types);

        let default_ret = if auto_ok.contains(&name) {
            quote!(std::result::Result::Ok(0))
        } else {
            quote!(std::result::Result::Err(-38))
        };

        let op_fn = quote! {
            fn #name (&mut self, #new_inputs) -> std::result::Result<i32, i32> {
                #default_ret
            }
        };
        let raw_op_fn = quote! {
            #unsafety #abi fn #name (#inputs) #output {
                #conversion

                let out = FileSystem::#name(
                    ((*fuse_get_context()).private_data as *mut Self).as_mut().expect("Corrupted context"),
                    #converted_call
                );

                match out {
                    Ok(o) => o,
                    Err(e) => e,
                }
            }
        };

        trait_fns.extend([op_fn]);
        raw_trait_fns.extend([raw_op_fn]);

        op_assignments.push(
            syn::parse(quote!(operations.#name = Some(<Self as FileSystemRaw>::#name);).into())
                .unwrap(),
        );
    }

    let op_assignments: Punctuated<Stmt, Semi> = op_assignments.into_iter().collect();

    let primitive_idents = [
        "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
    ];

    let reexport_list: Punctuated<Type, Comma> = all_reexport_types
        .into_iter()
        .filter_map(|s| (!primitive_idents.contains(&s.as_ref())).then(|| syn::parse::<Type>(s.parse().unwrap()).unwrap()))
        .collect();

    quote! {
        #[allow(unused_variables)]
        pub trait FileSystem: Sized {
            #trait_fns
        }
        pub trait FileSystemRaw: FileSystem {
            #raw_trait_fns
        }
        impl<F: FileSystem> FileSystemRaw for F {}
        
        pub trait FuseMain: FileSystemRaw + 'static {
            fn run(self, fuse_args: &[&str]) -> Result<(), i32>;
        }

        impl<F: FileSystemRaw + 'static> FuseMain for F {
            fn run(self, fuse_args: &[&str]) -> Result<(), i32> {
                let mut operations = crate::fuse_operations::default();
                #op_assignments

                let mut this = std::boxed::Box::new(self);

                let mut args_owned: std::vec::Vec<_> = fuse_args.into_iter().map(|s| std::ffi::CString::new(*s).unwrap()).collect();
                let mut args: std::vec::Vec<_> = args_owned.iter_mut().map(|cs| cs.as_ptr()).collect();

                let out = unsafe {
                    crate::fuse_main_real(
                        args.len() as i32,
                        args.as_mut_ptr() as *mut *mut std::os::raw::c_char,
                        &operations as *const crate::fuse_operations,
                        std::mem::size_of::<crate::fuse_operations>() as crate::size_t,
                        this.as_mut() as *mut Self as *mut std::ffi::c_void,
                    )
                };

                match out {
                    0 => Ok(()),
                    e => Err(e),
                }
            }
        }

        pub mod prelude {
            pub use crate::{
                FileSystem,
                FuseMain,
                #reexport_list
            };
        }
    
        #out
    }.into()
}
