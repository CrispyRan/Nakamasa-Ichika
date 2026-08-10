//! Nakamasa-proc 过程宏库

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, Token};
use syn::{
    ItemFn,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// 增强版路由宏，与 Salvo 完全兼容
/// 用法: #[route(GET, "/path")]
#[proc_macro_attribute]
pub fn route(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let attrs = parse_macro_input!(attr as RouteAttributes);

    let method = attrs.method.to_string();
    let path = attrs.path;
    let middlewares = attrs.middlewares;

    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_sig = &input_fn.sig;
    let fn_block = &input_fn.block;

    // 生成中间件链
    let middleware_chain = generate_middleware_chain(&middlewares);

    // 根据HTTP方法生成对应的路由方法调用
    let method_call = match method.as_str() {
        "GET" => quote! { .get(#fn_name) },
        "POST" => quote! { .post(#fn_name) },
        "PUT" => quote! { .put(#fn_name) },
        "DELETE" => quote! { .delete(#fn_name) },
        "PATCH" => quote! { .patch(#fn_name) },
        "HEAD" => quote! { .head(#fn_name) },
        "OPTIONS" => quote! { .options(#fn_name) },
        _ => panic!("不支持的HTTP方法: {}", method),
    };

    let expanded = quote! {
        #fn_vis #fn_sig #fn_block

        pub fn route() -> ::salvo::Router {
            let mut router = ::salvo::Router::with_path(#path);
            #(#middleware_chain)*
            router #method_call
        }
    };

    expanded.into()
}

/// 解析路由属性参数
struct RouteAttributes {
    method: Ident,
    path: String,
    middlewares: Vec<String>,
}

impl Parse for RouteAttributes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let method: Ident = input.parse()?;
        let _: Token![,] = input.parse()?;

        let path_lit: syn::LitStr = input.parse()?;
        let path = path_lit.value();

        let mut middlewares = Vec::new();

        while input.peek(Token![,]) {
            let _: Token![,] = input.parse()?;

            if input.is_empty() {
                break;
            }

            if input.peek(syn::Ident) && input.peek2(Token![=]) {
                let key: syn::Ident = input.parse()?;
                let _: Token![=] = input.parse()?;

                match key.to_string().as_str() {
                    "middleware" => {
                        if input.peek(syn::token::Bracket) {
                            let content;
                            syn::bracketed!(content in input);

                            while !content.is_empty() {
                                if content.peek(syn::LitStr) {
                                    let value: syn::LitStr = content.parse()?;
                                    middlewares.push(value.value());
                                } else if content.peek(syn::Ident) {
                                    let ident: syn::Ident = content.parse()?;
                                    middlewares.push(ident.to_string());
                                }

                                if content.is_empty() {
                                    break;
                                }
                                let _: Token![,] = content.parse()?;
                            }
                        } else {
                            let value: syn::LitStr = input.parse()?;
                            middlewares.push(value.value());
                        }
                    }
                    _ => {
                        let _: syn::LitStr = input.parse()?;
                    }
                }
            } else {
                break;
            }
        }

        Ok(RouteAttributes {
            method,
            path,
            middlewares,
        })
    }
}

/// 生成中间件链
fn generate_middleware_chain(middlewares: &[String]) -> Vec<proc_macro2::TokenStream> {
    middlewares
        .iter()
        .map(|mw| {
            let mw_ident = format_ident!("{}", mw);
            quote! {
                router = router.hoop(#mw_ident());
            }
        })
        .collect()
}

/// 控制器宏
#[proc_macro_attribute]
pub fn controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemMod);
    let attrs = parse_macro_input!(attr as ControllerAttributes);

    let prefix = attrs.prefix;
    let global_middlewares = attrs.middlewares;

    let mod_name = &input.ident;
    let mod_content = if let Some((_, items)) = input.content {
        items
    } else {
        Vec::new()
    };

    // 查找所有带有 #[route] 属性的函数
    let mut routes = Vec::new();

    for item in &mod_content {
        if let syn::Item::Fn(func) = item {
            for attr in &func.attrs {
                if attr.path().is_ident("route") {
                    routes.push(quote! {
                        super::#mod_name::route()
                    });
                }
            }
        }
    }

    // 生成全局中间件链
    let global_mw_chain = generate_middleware_chain(&global_middlewares);

    let expanded = quote! {
        mod #mod_name {
            use super::*;
            #(#mod_content)*

            pub fn routes() -> ::salvo::Router {
                let mut router = ::salvo::Router::new()
                    .path(#prefix);

                #(#global_mw_chain)*

                #(router.push(#routes);)*

                router
            }
        }
    };

    expanded.into()
}

/// 解析控制器属性
struct ControllerAttributes {
    prefix: String,
    middlewares: Vec<String>,
}

impl Parse for ControllerAttributes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut prefix = "/".to_string();
        let mut middlewares = Vec::new();

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            let _: Token![=] = input.parse()?;

            match key.to_string().as_str() {
                "prefix" => {
                    let value: syn::LitStr = input.parse()?;
                    prefix = value.value();
                }
                "middleware" => {
                    if input.peek(syn::token::Bracket) {
                        let content;
                        syn::bracketed!(content in input);

                        while !content.is_empty() {
                            if content.peek(syn::LitStr) {
                                let value: syn::LitStr = content.parse()?;
                                middlewares.push(value.value());
                            } else if content.peek(syn::Ident) {
                                let ident: syn::Ident = content.parse()?;
                                middlewares.push(ident.to_string());
                            }

                            if content.is_empty() {
                                break;
                            }
                            let _: Token![,] = content.parse()?;
                        }
                    } else {
                        let value: syn::LitStr = input.parse()?;
                        middlewares.push(value.value());
                    }
                }
                _ => {
                    let _: syn::LitStr = input.parse()?;
                }
            }

            if !input.is_empty() {
                let _: Token![,] = input.parse()?;
            }
        }

        Ok(ControllerAttributes {
            prefix,
            middlewares,
        })
    }
}

/// 增强版中间件宏，支持参数注入
#[proc_macro_attribute]
pub fn middleware(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let attrs = parse_macro_input!(attr as MiddlewareAttributes);

    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_sig = &input_fn.sig;
    let fn_block = &input_fn.block;
    let fn_async = input_fn.sig.asyncness.is_some();

    // 解析中间件属性
    let middleware_name = attrs.name.unwrap_or_else(|| fn_name.to_string());
    let inject_params = attrs.inject;

    let middleware_ident = format_ident!("{}", middleware_name);

    // 生成注入参数代码
    let inject_code = generate_inject_code(&inject_params);

    let expanded = if fn_async {
        quote! {
            #fn_vis #fn_sig #fn_block

            #fn_vis struct #middleware_ident;

            #[::salvo::async_trait]
            impl ::salvo::Handler for #middleware_ident {
                async fn handle(
                    &self,
                    req: &mut ::salvo::Request,
                    depot: &mut ::salvo::Depot,
                    res: &mut ::salvo::Response,
                    ctrl: &mut ::salvo::FlowCtrl,
                ) {
                    #inject_code
                    #fn_name(req, depot, res, ctrl).await
                }
            }
        }
    } else {
        quote! {
            #fn_vis #fn_sig #fn_block

            #fn_vis struct #middleware_ident;

            #[::salvo::async_trait]
            impl ::salvo::Handler for #middleware_ident {
                async fn handle(
                    &self,
                    req: &mut ::salvo::Request,
                    depot: &mut ::salvo::Depot,
                    res: &mut ::salvo::Response,
                    ctrl: &mut ::salvo::FlowCtrl,
                ) {
                    #inject_code
                    #fn_name(req, depot, res, ctrl)
                }
            }
        }
    };

    expanded.into()
}

/// 解析中间件属性
struct MiddlewareAttributes {
    name: Option<String>,
    inject: Vec<String>,
}

impl Parse for MiddlewareAttributes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut inject = Vec::new();

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            let _: Token![=] = input.parse()?;

            match key.to_string().as_str() {
                "name" => {
                    let value: syn::LitStr = input.parse()?;
                    name = Some(value.value());
                }
                "inject" => {
                    if input.peek(syn::token::Bracket) {
                        let content;
                        syn::bracketed!(content in input);

                        while !content.is_empty() {
                            if content.peek(syn::LitStr) {
                                let value: syn::LitStr = content.parse()?;
                                inject.push(value.value());
                            } else if content.peek(syn::Ident) {
                                let ident: syn::Ident = content.parse()?;
                                inject.push(ident.to_string());
                            }

                            if content.is_empty() {
                                break;
                            }
                            let _: Token![,] = content.parse()?;
                        }
                    } else {
                        let value: syn::LitStr = input.parse()?;
                        inject.push(value.value());
                    }
                }
                _ => {
                    let _: syn::LitStr = input.parse()?;
                }
            }

            if !input.is_empty() {
                let _: Token![,] = input.parse()?;
            }
        }

        Ok(MiddlewareAttributes { name, inject })
    }
}

/// 生成参数注入代码
fn generate_inject_code(params: &[String]) -> proc_macro2::TokenStream {
    if params.is_empty() {
        return quote! {};
    }

    let inject_statements: Vec<_> = params
        .iter()
        .map(|param| {
            let param_ident = format_ident!("{}", param);
            quote! {
                let #param_ident = depot.get::<#param_ident>().cloned().unwrap_or_default();
            }
        })
        .collect();

    quote! {
        #(#inject_statements)*
    }
}

/// 增强版请求验证器宏，支持字段级验证规则
#[proc_macro_derive(Validator, attributes(field))]
pub fn validator_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("Validator only supports structs with named fields"),
        },
        _ => panic!("Validator only supports structs"),
    };

    // 生成字段验证
    let field_checks: Vec<_> = fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_name_str = field_name.to_string();

            // 解析字段属性 - 使用正确的 syn 2.0 API
            let mut validation_rules = Vec::new();

            for attr in &field.attrs {
                if attr.path().is_ident("field") {
                    // 使用 parse_nested_meta 来解析属性
                    let _ = attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("rule") {
                            let value: syn::LitStr = meta.value()?.parse()?;
                            validation_rules.push(("rule".to_string(), value.value()));
                        }
                        Ok(())
                    });
                }
            }

            // 生成验证代码
            let validation_code =
                generate_validation_code(field_name, &field_name_str, &validation_rules);

            quote! {
                #validation_code
            }
        })
        .collect();

    let expanded = quote! {
        impl #struct_name {
            pub fn validate(&self) -> Result<(), Vec<String>> {
                let mut errors = Vec::new();

                #(#field_checks)*

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            }
        }
    };

    expanded.into()
}

/// 生成验证代码
fn generate_validation_code(
    field_name: &syn::Ident,
    field_name_str: &str,
    rules: &[(String, String)],
) -> proc_macro2::TokenStream {
    let field_value = quote! { &self.#field_name };

    let mut validation_checks = Vec::new();

    for (rule_name, rule_value) in rules {
        if rule_name.as_str() == "rule" {
            // 处理多条规则，如 "required|email|min:6"
            for rule in rule_value.split('|') {
                let rule_parts: Vec<&str> = rule.splitn(2, ':').collect();
                let (rule_type, rule_param) = if rule_parts.len() > 1 {
                    (rule_parts[0], Some(rule_parts[1]))
                } else {
                    (rule_parts[0], None)
                };

                let check = match rule_type {
                    "required" => quote! {
                        if #field_value.is_empty() {
                            errors.push(format!("Field '{}' is required", #field_name_str));
                        }
                    },
                    "email" => quote! {
                        if !#field_value.is_empty() && !#field_value.contains('@') {
                            errors.push(format!("Field '{}' must be a valid email", #field_name_str));
                        }
                    },
                    "min" => {
                        if let Some(param) = rule_param {
                            let min_val: i32 = param.parse().unwrap_or(0);
                            quote! {
                                if let Ok(num) = #field_value.parse::<i32>() {
                                    if num < #min_val {
                                        errors.push(format!("Field '{}' must be at least {}", #field_name_str, #min_val));
                                    }
                                } else if #field_value.len() < #min_val as usize {
                                    errors.push(format!("Field '{}' must be at least {} characters", #field_name_str, #min_val));
                                }
                            }
                        } else {
                            quote! {}
                        }
                    }
                    "max" => {
                        if let Some(param) = rule_param {
                            let max_val: i32 = param.parse().unwrap_or(0);
                            quote! {
                                if let Ok(num) = #field_value.parse::<i32>() {
                                    if num > #max_val {
                                        errors.push(format!("Field '{}' must be at most {}", #field_name_str, #max_val));
                                    }
                                } else if #field_value.len() > #max_val as usize {
                                    errors.push(format!("Field '{}' must be at most {} characters", #field_name_str, #max_val));
                                }
                            }
                        } else {
                            quote! {}
                        }
                    }
                    _ => quote! {},
                };

                validation_checks.push(check);
            }
        }
    }

    quote! {
        #(#validation_checks)*
    }
}
