#![doc = include_str!("../README.md")]
#![doc(
    test(attr(deny(warnings))),
    html_favicon_url = "https://raw.githubusercontent.com/helsing-ai/twurst/main/docs/img/twurst.png",
    html_logo_url = "https://raw.githubusercontent.com/helsing-ai/twurst/main/docs/img/twurst.png"
)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

use self::proto_path_map::ProtoPathMap;
pub use prost_build as prost;
use prost_build::{Config, Module, Service, ServiceGenerator};
use regex::Regex;
use std::collections::HashSet;
use std::fmt::Write;
use std::io::{Error, Result};
use std::path::{Path, PathBuf};
use std::{env, fs};

mod proto_path_map;

/// Builds protobuf bindings for Twirp.
///
/// Client and server are not enabled by defaults and must be enabled with the [`with_client`](Self::with_client) and [`with_server`](Self::with_server) methods.
#[derive(Default)]
pub struct TwirpBuilder {
    config: Config,
    generator: TwirpServiceGenerator,
    type_name_domain: Option<String>,
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
        prost_reflect_build::Builder::new()
            .file_descriptor_set_bytes("self::FILE_DESCRIPTOR_SET_BYTES")
            .configure(&mut self.config, protos, includes)?;

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

        // TODO(vsiles) consider proper AST parsing in case we need to do something
        // more robust
        //
        // prepare a regex to match `pub mod <module-name> {`
        let re = Regex::new(r"^(\s*)pub mod \w+ \{\s*$").expect("Failed to compile regex");

        // We add the file descriptor to every file to make reflection work automatically
        for module in modules {
            let file_path = Path::new(&out_dir).join(module.to_file_name_or("_"));
            if !file_path.exists() {
                continue; // We ignore not built files
            }
            let original_content = fs::read_to_string(&file_path)?;

            // scan for nested modules and insert the right FILE_DESCRIPTOR_SET_BYTES definition
            let mut modified_content = original_content
                .lines()
                .flat_map(|line| {
                    if let Some(captures) = re.captures(line) {
                        let indentation = captures.get(1).map_or("", |m| m.as_str());
                        vec![
                            line.to_string(),
                            // if there is no nested type, the next line would generate a warning
                            format!("    {}{}", indentation, "#[allow(unused_imports)]"),
                            format!(
                                "    {}{}",
                                indentation, "use super::FILE_DESCRIPTOR_SET_BYTES;"
                            ),
                        ]
                    } else {
                        vec![line.to_string()]
                    }
                })
                .collect::<Vec<_>>();

            modified_content.push("const FILE_DESCRIPTOR_SET_BYTES: &[u8] = include_bytes!(\"file_descriptor_set.bin\");\n".to_string());
            let file_content = modified_content.join("\n");

            fs::write(&file_path, &file_content)?;
        }

        Ok(())
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
        self.do_generate(service, buf)
            .expect("failed to generate Twirp service")
    }
}

impl TwirpServiceGenerator {
    fn do_generate(&mut self, service: Service, buf: &mut String) -> std::fmt::Result {
        let mut service_matches = self
            .matched_request_extractors
            .service_matches(&service)
            .peekable();

        let extractors: Vec<_> = if service_matches.peek().is_some() {
            service_matches.collect()
        } else {
            self.default_request_extractors.iter().collect()
        };

        if self.client {
            writeln!(buf)?;
            for comment in &service.comments.leading {
                writeln!(buf, "/// {comment}")?;
            }
            if service.options.deprecated.unwrap_or(false) {
                writeln!(buf, "#[deprecated]")?;
            }
            writeln!(buf, "#[derive(Clone)]")?;
            writeln!(
                buf,
                "pub struct {}Client<C: ::twurst_client::TwirpHttpService> {{",
                service.name
            )?;
            writeln!(buf, "    client: ::twurst_client::TwirpHttpClient<C>")?;
            writeln!(buf, "}}")?;
            writeln!(buf)?;
            writeln!(
                buf,
                "impl<C: ::twurst_client::TwirpHttpService> {}Client<C> {{",
                service.name
            )?;
            writeln!(
                buf,
                "    pub fn new(client: impl Into<::twurst_client::TwirpHttpClient<C>>) -> Self {{"
            )?;
            writeln!(buf, "        Self {{ client: client.into() }}")?;
            writeln!(buf, "    }}")?;
            for method in &service.methods {
                if method.client_streaming || method.server_streaming {
                    continue; // Not supported
                }
                for comment in &method.comments.leading {
                    writeln!(buf, "    /// {comment}")?;
                }
                if method.options.deprecated.unwrap_or(false) {
                    writeln!(buf, "#[deprecated]")?;
                }
                writeln!(
                    buf,
                    "    pub async fn {}(&self, request: &{}) -> Result<{}, ::twurst_client::TwirpError> {{",
                    method.name, method.input_type, method.output_type,
                )?;
                writeln!(
                    buf,
                    "        self.client.call(\"/{}.{}/{}\", request).await",
                    service.package, service.proto_name, method.proto_name,
                )?;
                writeln!(buf, "    }}")?;
            }
            writeln!(buf, "}}")?;
        }

        if self.server {
            writeln!(buf)?;
            for comment in &service.comments.leading {
                writeln!(buf, "/// {comment}")?;
            }
            writeln!(buf, "#[::twurst_server::codegen::trait_variant_make(Send)]")?;
            writeln!(buf, "pub trait {} {{", service.name)?;
            for method in &service.methods {
                if !self.grpc && (method.client_streaming || method.server_streaming) {
                    continue; // No streaming
                }
                for comment in &method.comments.leading {
                    writeln!(buf, "    /// {comment}")?;
                }
                write!(buf, "    async fn {}(&self, request: ", method.name)?;
                if method.client_streaming {
                    write!(
                        buf,
                        "impl ::twurst_server::codegen::Stream<Item=Result<{},::twurst_client::TwirpError>> + Send + 'static",
                        method.input_type,
                    )?;
                } else {
                    write!(buf, "{}", method.input_type)?;
                }
                for (arg_name, arg_type) in &extractors {
                    write!(buf, ", {arg_name}: {arg_type}")?;
                }
                writeln!(buf, ") -> Result<")?;
                if method.server_streaming {
                    // TODO: move back to `impl` when we will be able to use precise capturing to not capture &self
                    writeln!(
                        buf,
                        "Box<dyn ::twurst_server::codegen::Stream<Item=Result<{}, ::twurst_server::TwirpError>> + Send>",
                        method.output_type
                    )?;
                } else {
                    writeln!(buf, "{}", method.output_type)?;
                }
                writeln!(buf, ", ::twurst_server::TwirpError>;")?;
            }
            writeln!(buf)?;
            writeln!(
                buf,
                "    fn into_router<S: Clone + Send + Sync + 'static>(self) -> ::twurst_server::codegen::Router<S> where Self : Sized + Send + Sync + 'static {{"
            )?;
            writeln!(
                buf,
                "        ::twurst_server::codegen::TwirpRouter::new(::std::sync::Arc::new(self))"
            )?;
            for method in &service.methods {
                if method.client_streaming || method.server_streaming {
                    writeln!(
                        buf,
                        "            .route_streaming(\"/{}.{}/{}\")",
                        service.package, service.proto_name, method.proto_name,
                    )?;
                    continue;
                }
                write!(
                    buf,
                    "            .route(\"/{}.{}/{}\", |service: ::std::sync::Arc<Self>, request: {}",
                    service.package, service.proto_name, method.proto_name, method.input_type,
                )?;
                if extractors.is_empty() {
                    write!(buf, ", _: ::twurst_server::codegen::RequestParts, _: S")?;
                } else {
                    write!(
                        buf,
                        ", mut parts: ::twurst_server::codegen::RequestParts, state: S",
                    )?;
                }
                write!(buf, "| {{")?;
                writeln!(buf, "                async move {{")?;
                write!(buf, "                    service.{}(request", method.name)?;
                for (_name, type_name) in &extractors {
                    write!(
                        buf,
                        ", match <{type_name} as ::twurst_server::codegen::FromRequestParts<_>>::from_request_parts(&mut parts, &state).await {{ Ok(r) => r, Err(e) => {{ return Err(::twurst_server::codegen::twirp_error_from_response(e).await) }} }}"
                    )?;
                }
                writeln!(buf, ").await")?;
                writeln!(buf, "                }}")?;
                writeln!(buf, "            }})")?;
            }
            writeln!(buf, "            .build()")?;
            writeln!(buf, "    }}")?;

            if self.grpc {
                writeln!(buf)?;
                writeln!(
                    buf,
                    "    fn into_grpc_router(self) -> ::twurst_server::codegen::Router where Self : Sized + Send + Sync + 'static {{"
                )?;
                writeln!(
                    buf,
                    "        ::twurst_server::codegen::GrpcRouter::new(::std::sync::Arc::new(self))"
                )?;
                for method in &service.methods {
                    let method_name = match (method.client_streaming, method.server_streaming) {
                        (false, false) => "route",
                        (false, true) => "route_server_streaming",
                        (true, false) => "route_client_streaming",
                        (true, true) => "route_streaming",
                    };
                    write!(
                        buf,
                        "            .{}(\"/{}.{}/{}\", |service: ::std::sync::Arc<Self>, request: ",
                        method_name, service.package, service.proto_name, method.proto_name,
                    )?;
                    if method.client_streaming {
                        write!(
                            buf,
                            "::twurst_server::codegen::GrpcClientStream<{}>",
                            method.input_type,
                        )?;
                    } else {
                        write!(buf, "{}", method.input_type)?;
                    }
                    if extractors.is_empty() {
                        write!(buf, ", _: ::twurst_server::codegen::RequestParts")?;
                    } else {
                        write!(buf, ", mut parts: ::twurst_server::codegen::RequestParts")?;
                    }
                    write!(buf, "| {{")?;
                    write!(buf, "                async move {{")?;
                    if method.server_streaming {
                        write!(buf, "Ok(Box::into_pin(")?;
                    }
                    write!(buf, "service.{}(request", method.name)?;
                    for (_name, type_name) in &extractors {
                        write!(
                            buf,
                            ", match <{type_name} as ::twurst_server::codegen::FromRequestParts<_>>::from_request_parts(&mut parts, &()).await {{ Ok(r) => r, Err(e) => {{ return Err(::twurst_server::codegen::twirp_error_from_response(e).await) }} }}"
                        )?;
                    }
                    write!(buf, ").await")?;
                    if method.server_streaming {
                        write!(buf, "?))")?;
                    }
                    writeln!(buf, "}}")?;
                    writeln!(buf, "            }})")?;
                }
                writeln!(buf, "            .build()")?;
                writeln!(buf, "    }}")?;
            }

            writeln!(buf, "}}")?;
        }

        Ok(())
    }
}
