#![doc = include_str!("../README.md")]
#![doc(
    test(attr(deny(warnings))),
    html_favicon_url = "https://raw.githubusercontent.com/helsing-ai/twurst/main/docs/img/twurst.png",
    html_logo_url = "https://raw.githubusercontent.com/helsing-ai/twurst/main/docs/img/twurst.png"
)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

use self::proto_path_map::ProtoPathMap;
use prettyplease::unparse;
use proc_macro2::TokenStream;
pub use prost_build as prost;
use prost_build::{Comments, Config, Module, Service, ServiceGenerator};
use quote::{format_ident, quote};
use std::collections::HashSet;
use std::fmt::Write;
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};
use std::{env, fs};
use syn::{Item, parse_quote};

mod proto_path_map;

/// Builds protobuf bindings for Twirp.
///
/// Client and server are not enabled by defaults and must be enabled with the [`with_client`](Self::with_client) and [`with_server`](Self::with_server) methods.
#[derive(Default)]
pub struct TwirpBuilder {
    config: Config,
    generator: TwirpServiceGenerator,
    type_name_domain: Option<String>,
    skip_prost_reflect: bool,
}

impl TwirpBuilder {
    /// Builder with the default prost [`Config`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder with a custom prost [`Config`].
    pub fn from_prost(config: Config) -> Self {
        Self {
            config,
            generator: TwirpServiceGenerator::new(),
            type_name_domain: None,
            skip_prost_reflect: false,
        }
    }

    /// Generates code for the Twirp client.
    pub fn with_client(mut self) -> Self {
        self.generator = self.generator.with_client();
        self
    }

    /// Generates code for the Twirp server.
    pub fn with_server(mut self) -> Self {
        self.generator = self.generator.with_server();
        self
    }

    /// Generates code for gRPC alongside Twirp.
    pub fn with_grpc(mut self) -> Self {
        self.generator = self.generator.with_grpc();
        self
    }

    #[deprecated(
        since = "0.3.1",
        note = "replaced with with_default_axum_request_extractor"
    )]
    pub fn with_axum_request_extractor(
        self,
        name: impl Into<String>,
        type_name: impl Into<String>,
    ) -> Self {
        self.with_default_axum_request_extractor(name, type_name)
    }

    /// Adds an extra parameter to generated server methods that implements [`axum::FromRequestParts`](https://docs.rs/axum/latest/axum/extract/trait.FromRequestParts.html).
    ///
    /// For example
    /// ```proto
    /// message Service {
    ///     rpc Test(TestRequest) returns (TestResponse) {}
    /// }
    /// ```
    /// Compiled with option `.with_default_axum_request_extractor("headers", "::axum::http::HeaderMap")`
    /// will generate the following code (in every service) allowing to extract the request headers:
    /// ```ignore
    /// trait Service {
    ///     async fn test(request: TestRequest, headers: ::axum::http::HeaderMap) -> Result<TestResponse, TwirpError>;
    /// }
    /// ```
    ///
    /// Note that the parameter type must implement [`axum::FromRequestParts`](https://docs.rs/axum/latest/axum/extract/trait.FromRequestParts.html).
    ///
    /// There is a companion method to this: [`TwirpBuilder::with_service_specific_axum_request_extractor`], which adds request extractors per service,
    /// rather than for all services given to the build.
    pub fn with_default_axum_request_extractor(
        mut self,
        name: impl Into<String>,
        type_name: impl Into<String>,
    ) -> Self {
        self.generator = self
            .generator
            .with_default_axum_request_extractor(name, type_name);
        self
    }

    /// Adds an extra parameter to a service's server methods that implements [`axum::FromRequestParts`](https://docs.rs/axum/latest/axum/extract/trait.FromRequestParts.html).
    ///
    /// For example, given:
    /// ```proto
    /// message ServiceA {
    ///     rpc Test(TestRequest) returns (TestResponse) {}
    /// }
    /// ```
    ///
    /// And:
    ///
    /// ```proto
    /// message ServiceB {
    ///     rpc Test(TestRequest) returns (TestResponse) {}
    /// }
    /// ```
    ///
    /// When compiled with option `.with_service_specific_axum_request_extractor("headers", "::axum::http::HeaderMap", "ServiceA")`
    /// will generate the following code extract the request headers in just implementors of `ServiceA`:
    /// ```ignore
    /// trait ServiceA {
    ///     async fn test(request: TestRequest, headers: ::axum::http::HeaderMap) -> Result<TestResponse, TwirpError>;
    /// }
    ///
    /// trait ServiceB {
    ///     async fn test(request: TestRequest) -> Result<TestResponse, TwirpError>;
    /// }
    /// ```
    ///
    /// Note that the parameter type must implement [`axum::FromRequestParts`](https://docs.rs/axum/latest/axum/extract/trait.FromRequestParts.html).
    ///
    /// Service specific request extractors will overwrite any that are set by: [`TwirpBuilder::with_default_axum_request_extractor`]. They are NOT additive, but you can
    /// add any default extractors also as service specific ones, for example:
    /// ```ignore
    /// let builder = TwirpBuilder::new()
    ///     .with_server()
    ///     .with_default_axum_request_extractor(
    ///         "auth_header",
    ///         "my_crate::AuthorizationHeader",
    ///     )
    ///     .with_service_specific_axum_request_extractor(
    ///         "auth_header",
    ///         "my_crate::AuthorizationHeader",
    ///         "MyService"
    ///     );
    ///     .with_service_specific_axum_request_extractor(
    ///         "request_context",
    ///         "my_crate::RequestContext",
    ///         "MyService"
    ///     );
    /// ```
    /// Will generate traits for `MyService` which extract both `auth_header` and
    /// `request_context`, whilst all others will just have `auth_header`.
    ///
    /// The service should be specified by Proto path. For example:
    ///
    /// ```ignore
    /// let mut builder = TwirpBuilder::new().with_server();
    ///
    /// // Match any Service called `MyService`
    /// builder.with_service_specific_axum_request_extractor(
    ///     "auth_header",
    ///     "my_crate::AuthorizationHeader",
    ///     "MyService"
    /// );
    ///
    /// // Match any Service called `MyService` in the package `MyPackage`
    /// builder.with_service_specific_axum_request_extractor(
    ///     "auth_header",
    ///     "my_crate::AuthorizationHeader",
    ///     ".MyPackage.MyService"
    /// );
    ///
    /// // Match all Services in the package `MyPackage`
    /// builder.with_service_specific_axum_request_extractor(
    ///     "auth_header",
    ///     "my_crate::AuthorizationHeader",
    ///     ".MyPackage"
    /// );
    ///
    /// // Match _any_ Service.
    /// //
    /// // NOTE: This will override the defaults for ALL services. This is useful if you want all
    /// // services to have an extractor with a subset having additional ones, however it means you cannot
    /// // have disjoint sets of extractors across services.
    /// builder.with_service_specific_axum_request_extractor(
    ///     "auth_header",
    ///     "my_crate::AuthorizationHeader",
    ///     "."
    /// );
    pub fn with_service_specific_axum_request_extractor(
        mut self,
        name: impl Into<String>,
        type_name: impl Into<String>,
        service_path: impl Into<String>,
    ) -> Self {
        self.generator = self.generator.with_service_specific_axum_request_extractor(
            name,
            type_name,
            service_path,
        );
        self
    }

    /// Skips the built-in prost-reflect configuration and file patching.
    ///
    /// When enabled, callers are responsible for configuring prost-reflect
    /// on the [`Config`] before passing it to [`from_prost`](Self::from_prost).
    /// This is useful when using a custom `out_dir` or when using
    /// `descriptor_pool` mode instead of `file_descriptor_set_bytes`.
    pub fn skip_prost_reflect(mut self) -> Self {
        self.skip_prost_reflect = true;
        self
    }

    /// Customizes the type name domain.
    ///
    /// By default, 'type.googleapis.com' is used.
    pub fn with_type_name_domain(mut self, domain: impl Into<String>) -> Self {
        self.type_name_domain = Some(domain.into());
        self
    }

    /// Do compile the protos.
    pub fn compile_protos(
        mut self,
        protos: &[impl AsRef<Path>],
        includes: &[impl AsRef<Path>],
    ) -> Result<()> {
        let out_dir = PathBuf::from(
            env::var_os("OUT_DIR").ok_or_else(|| Error::other("OUT_DIR is not set"))?,
        );

        // We make sure the script is executed again if a file changed
        for proto in protos {
            println!("cargo:rerun-if-changed={}", proto.as_ref().display());
        }

        self.config
            .enable_type_names()
            .type_name_domain(
                ["."],
                self.type_name_domain
                    .as_deref()
                    .unwrap_or("type.googleapis.com"),
            )
            .service_generator(Box::new(self.generator));

        // We configure with prost reflect
        if !self.skip_prost_reflect {
            prost_reflect_build::Builder::new()
                .file_descriptor_set_bytes("self::FILE_DESCRIPTOR_SET_BYTES")
                .configure(&mut self.config, protos, includes)?;
        }

        // We do the build itself while saving the list of modules
        let config = self.config.skip_protoc_run();
        let file_descriptor_set = config.load_fds(protos, includes)?;
        let modules = file_descriptor_set
            .file
            .iter()
            .map(|fd| Module::from_protobuf_package_name(fd.package()))
            .collect::<HashSet<_>>();

        // We generate the files
        config.compile_fds(file_descriptor_set)?;

        // We add the file descriptor to every file to make reflection work automatically
        if !self.skip_prost_reflect {
            for module in modules {
                let file_path = Path::new(&out_dir).join(module.to_file_name_or("_"));
                if !file_path.exists() {
                    continue; // We ignore not built files
                }
                let original_content = fs::read_to_string(&file_path)?;
                let modified_content = add_use_file_descriptor_to_file(&original_content)?;
                fs::write(&file_path, &modified_content)?;
            }
        }

        Ok(())
    }
}

fn add_use_file_descriptor_to_file(file: &str) -> Result<String> {
    let mut ast = syn::parse_file(file).map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
    add_use_file_descriptor_to_nested_modules(&mut ast.items);
    ast.items.push(parse_quote! {
        const FILE_DESCRIPTOR_SET_BYTES: &[u8] = include_bytes!("file_descriptor_set.bin");
    });
    Ok(unparse(&ast))
}

fn add_use_file_descriptor_to_nested_modules(items: &mut Vec<Item>) {
    for item in items {
        if let Item::Mod(module) = item {
            if let Some((_, module_content)) = &mut module.content {
                module_content.insert(
                    0,
                    parse_quote! {
                        #[allow(unused_imports)]
                        use super::FILE_DESCRIPTOR_SET_BYTES;
                    },
                );
            }
        }
    }
}

/// Low level generator for Twirp related code.
///
/// This only useful if you want to customize builds. For common use cases, please use [`TwirpBuilder`].
///
/// Should be given to [`Config::service_generator`].
///
/// Client and server are not enabled by defaults and must be enabled with the [`with_client`](Self::with_client) and [`with_server`](Self::with_server) methods.
#[derive(Default)]
struct TwirpServiceGenerator {
    client: bool,
    server: bool,
    grpc: bool,
    // stores the default extractors as (argument_name, extractor_type)
    default_request_extractors: Vec<(String, String)>,
    // stores an extractor for a proto path as (argument_name, extractor_type)
    matched_request_extractors: ProtoPathMap<(String, String)>,
}

impl TwirpServiceGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_client(mut self) -> Self {
        self.client = true;
        self
    }

    pub fn with_server(mut self) -> Self {
        self.server = true;
        self
    }

    pub fn with_grpc(mut self) -> Self {
        self.grpc = true;
        self
    }

    pub fn with_default_axum_request_extractor(
        mut self,
        name: impl Into<String>,
        type_name: impl Into<String>,
    ) -> Self {
        self.default_request_extractors
            .push((name.into(), type_name.into()));
        self
    }

    // This will override any and all default extractors, but only for the services which match service_proto_path.
    pub fn with_service_specific_axum_request_extractor(
        mut self,
        name: impl Into<String>,
        type_name: impl Into<String>,
        service_proto_path: impl Into<String>,
    ) -> Self {
        self.matched_request_extractors
            .insert(service_proto_path.into(), (name.into(), type_name.into()));
        self
    }
}

impl ServiceGenerator for TwirpServiceGenerator {
    fn generate(&mut self, service: Service, buf: &mut String) {
        let mut service_matches = self
            .matched_request_extractors
            .service_matches(&service)
            .peekable();
        let extractors = if service_matches.peek().is_some() {
            service_matches.collect::<Vec<_>>()
        } else {
            self.default_request_extractors.iter().collect()
        };

        let extractor_names = extractors
            .iter()
            .map(|(n, _)| format_ident!("{n}"))
            .collect::<Vec<_>>();
        let extractor_types = extractors
            .iter()
            .map(|(_, t)| t.parse().unwrap())
            .collect::<Vec<TokenStream>>();

        let mut output = TokenStream::new();
        if self.client {
            let client_name = format_ident!("{}Client", service.name);
            let service_docs = quote_comments(&service.comments);
            let service_deprecated = if service.options.deprecated.unwrap_or(false) {
                Some(quote! { #[deprecated] })
            } else {
                None
            };

            let method_tokens = service
                .methods
                .iter()
                .filter(|m| !m.client_streaming && !m.server_streaming)
                .map(|method| {
                    let method_ident = format_ident!("{}", method.name);
                    let input_type: TokenStream = method.input_type.parse().unwrap();
                    let output_type: TokenStream = method.output_type.parse().unwrap();
                    let route = format!(
                        "/{}.{}/{}",
                        service.package, service.proto_name, method.proto_name
                    );
                    let method_docs = quote_comments(&method.comments);
                    let method_deprecated = if method.options.deprecated.unwrap_or(false) {
                        quote! { #[deprecated] }
                    } else {
                        quote! {}
                    };
                    quote! {
                        #(#method_docs)*
                        #method_deprecated
                        pub async fn #method_ident(&self, request: &#input_type) -> Result<#output_type, ::twurst_client::TwirpError> {
                            self.client.call(#route, request).await
                        }
                    }
                })
                .collect::<Vec<_>>();

            output.extend(quote! {
                #(#service_docs)*
                #service_deprecated
                #[derive(Clone)]
                pub struct #client_name<C: ::twurst_client::TwirpHttpService> {
                    client: ::twurst_client::TwirpHttpClient<C>,
                }

                impl<C: ::twurst_client::TwirpHttpService> #client_name<C> {
                    pub fn new(client: impl Into<::twurst_client::TwirpHttpClient<C>>) -> Self {
                        Self { client: client.into() }
                    }
                    #(#method_tokens)*
                }
            });
        }

        if self.server {
            let service_name_ident = format_ident!("{}", service.name);

            let service_docs = quote_comments(&service.comments);

            let trait_method_tokens = service
                .methods
                .iter()
                .filter(|m| self.grpc || (!m.client_streaming && !m.server_streaming))
                .map(|method| {
                    let method_ident = format_ident!("{}", method.name);
                    let input_type: TokenStream = method.input_type.parse().unwrap();
                    let output_type: TokenStream = method.output_type.parse().unwrap();
                    let method_docs = quote_comments(&method.comments);
                    let request_param = if method.client_streaming {
                        quote! {
                            impl ::twurst_server::codegen::Stream<Item=Result<#input_type,::twurst_client::TwirpError>> + Send + 'static
                        }
                    } else {
                        input_type
                    };

                    // TODO: move back to `impl` when we will be able to use precise capturing to not capture &self
                    let return_type = if method.server_streaming {
                        quote! {
                            Box<dyn ::twurst_server::codegen::Stream<Item=Result<#output_type, ::twurst_server::TwirpError>> + Send>
                        }
                    } else {
                        output_type
                    };

                    quote! {
                        #(#method_docs)*
                        async fn #method_ident(&self, request: #request_param #(, #extractor_names: #extractor_types)*) -> Result<#return_type, ::twurst_server::TwirpError>;
                    }
                })
                .collect::<Vec<_>>();

            let router_route_tokens = service
                .methods
                .iter()
                .map(|method| {
                    let route = format!(
                        "/{}.{}/{}",
                        service.package, service.proto_name, method.proto_name
                    );
                    let method_ident = format_ident!("{}", method.name);
                    let input_type: TokenStream = method.input_type.parse().unwrap();

                    if method.client_streaming || method.server_streaming {
                        quote! { .route_streaming(#route) }
                    } else {
                        let (parts_param, state_param) = if extractors.is_empty() {
                            (
                                quote! { _: ::twurst_server::codegen::RequestParts },
                                quote! { _: S },
                            )
                        } else {
                            (
                                quote! { mut parts: ::twurst_server::codegen::RequestParts },
                                quote! { state: S },
                            )
                        };

                        let ext_types = extractor_types.clone();

                        quote! {
                            .route(#route, |service: ::std::sync::Arc<Self>, request: #input_type, #parts_param, #state_param| {
                                async move {
                                    service.#method_ident(request #(, match <#ext_types as ::twurst_server::codegen::FromRequestParts<_>>::from_request_parts(&mut parts, &state).await { Ok(r) => r, Err(e) => { return Err(::twurst_server::codegen::twirp_error_from_response(e).await) } })*).await
                                }
                            })
                        }
                    }
                })
                .collect::<Vec<_>>();

            let grpc_router_tokens = if self.grpc {
                let grpc_route_tokens = service
                    .methods
                    .iter()
                    .map(|method| {
                        let route = format!(
                            "/{}.{}/{}",
                            service.package, service.proto_name, method.proto_name
                        );
                        let method_ident = format_ident!("{}", method.name);
                        let input_type: TokenStream = method.input_type.parse().unwrap();
                        let grpc_fn_ident =
                            match (method.client_streaming, method.server_streaming) {
                                (false, false) => format_ident!("route"),
                                (false, true) => format_ident!("route_server_streaming"),
                                (true, false) => format_ident!("route_client_streaming"),
                                (true, true) => format_ident!("route_streaming"),
                            };
                        let request_type = if method.client_streaming {
                            quote! { ::twurst_server::codegen::GrpcClientStream<#input_type> }
                        } else {
                            input_type
                        };
                        let parts_param = if extractors.is_empty() {
                            quote! { _: ::twurst_server::codegen::RequestParts }
                        } else {
                            quote! { mut parts: ::twurst_server::codegen::RequestParts }
                        };
                        let service_call = quote! {
                                service.#method_ident(request #(, match <#extractor_types as ::twurst_server::codegen::FromRequestParts<_>>::from_request_parts(&mut parts, &()).await { Ok(r) => r, Err(e) => { return Err(::twurst_server::codegen::twirp_error_from_response(e).await) } })*).await
                            };
                        let service_call = if method.server_streaming {
                            quote! { Ok(Box::into_pin(#service_call?)) }
                        } else {
                            service_call
                        };
                        quote! {
                            .#grpc_fn_ident(#route, |service: ::std::sync::Arc<Self>, request: #request_type, #parts_param| {
                                async move {
                                    #service_call
                                }
                            })
                        }
                    })
                    .collect::<Vec<_>>();

                Some(quote! {
                    fn into_grpc_router(self) -> ::twurst_server::codegen::Router where Self: Sized + Send + Sync + 'static {
                        ::twurst_server::codegen::GrpcRouter::new(::std::sync::Arc::new(self))
                        #(#grpc_route_tokens)*
                        .build()
                    }
                })
            } else {
                None
            };

            output.extend(quote! {
                #(#service_docs)*
                #[::twurst_server::codegen::trait_variant_make(Send)]
                pub trait #service_name_ident {
                    #(#trait_method_tokens)*

                    fn into_router<S: Clone + Send + Sync + 'static>(self) -> ::twurst_server::codegen::Router<S> where Self: Sized + Send + Sync + 'static {
                        ::twurst_server::codegen::TwirpRouter::new(::std::sync::Arc::new(self))
                        #(#router_route_tokens)*
                        .build()
                    }

                    #grpc_router_tokens
                }
            });
        }

        if !output.is_empty() {
            buf.push('\n');
            write!(buf, "{output}").unwrap()
        }
    }
}

fn quote_comments(comments: &Comments) -> Vec<TokenStream> {
    comments
        .leading
        .iter()
        .map(|c| quote! { #[doc = #c] })
        .collect::<Vec<_>>()
}
