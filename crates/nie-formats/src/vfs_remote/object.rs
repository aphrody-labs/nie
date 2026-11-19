//! Object-store game sources (`r2://`, `s3://`, `gdrive://`, `rclone://`, `https://`) over
//! `opendal`: ranged reads with `Operator::read_with(..).range(..)`, no `rclone` binary.

use super::rclone_conf::{ObjectSpec, Service};
use super::{MAX_READ, Result};
use opendal::layers::RetryLayer;
use opendal::services::{Gdrive, Http, S3};
use opendal::{ErrorKind, Operator};

pub(super) struct Object {
    op: Operator,
    /// URL path of an HTTP source: directories are listed from the server's HTML index.
    http_root: Option<String>,
}

fn io(error: opendal::Error) -> String {
    error.to_string()
}

impl Object {
    pub fn open(spec: &ObjectSpec) -> Result<Self> {
        // The HTTP transport of every service, reqwest on the nie rustls provider (the `opendal`
        // facade installs its own only with the aws-lc feature); installing it twice is harmless.
        nie_tls::install();
        opendal::install_default();
        opendal_http_transport_reqwest::install_default();
        let op = match &spec.service {
            Service::S3 {
                bucket,
                endpoint,
                region,
                access_key_id,
                secret_access_key,
                session_token,
                env_auth,
            } => {
                let mut builder = S3::default().bucket(bucket).root(&spec.root);
                if let Some(endpoint) = endpoint {
                    builder = builder.endpoint(endpoint);
                }
                if let Some(region) = region {
                    builder = builder.region(region);
                }
                if let (Some(id), Some(secret)) = (access_key_id, secret_access_key) {
                    builder = builder.access_key_id(id).secret_access_key(secret);
                }
                if let Some(token) = session_token {
                    builder = builder.session_token(token);
                }
                if !env_auth {
                    builder = builder.disable_config_load().disable_ec2_metadata();
                    if access_key_id.is_none() {
                        builder = builder.skip_signature();
                    }
                }
                Operator::new(builder)
            }
            Service::Drive {
                client_id,
                client_secret,
                access_token,
                refresh_token,
            } => {
                let mut builder = Gdrive::default().root(&spec.root);
                if let Some(token) = access_token {
                    builder = builder.access_token(token);
                }
                if let (Some(refresh), Some(id), Some(secret)) =
                    (refresh_token, client_id, client_secret)
                {
                    builder = builder
                        .refresh_token(refresh)
                        .client_id(id)
                        .client_secret(secret);
                }
                Operator::new(builder)
            }
            Service::Http { endpoint } => {
                let mut builder = Http::default().endpoint(endpoint).root(&spec.root);
                if let Ok(token) = std::env::var("NIE_HTTP_AUTH_TOKEN") {
                    builder = builder.token(&token);
                }
                Operator::new(builder)
            }
        }
        .map_err(io)?
        .layer(RetryLayer::new());
        let http_root = matches!(spec.service, Service::Http { .. }).then(|| spec.root.clone());
        Ok(Self { op, http_root })
    }

    /// `https://host[:port]/root`: plain HTTPS with range reads and HTML directory indexes.
    pub fn https(endpoint: &str, root: &str) -> Result<Self> {
        Self::open(&ObjectSpec {
            service: Service::Http {
                endpoint: endpoint.to_owned(),
            },
            root: root.to_owned(),
        })
    }

    pub async fn read(&self, path: &str, range: Option<(u64, usize)>) -> Result<Vec<u8>> {
        let data = match range {
            Some((offset, length)) => {
                let data = self
                    .op
                    .read_with(path)
                    .range(offset..offset + length as u64)
                    .await
                    .map_err(io)?
                    .to_vec();
                if data.len() != length {
                    return Err(format!(
                        "range read of {path} truncated: wanted {length} bytes, got {}",
                        data.len()
                    ));
                }
                data
            }
            None => {
                let data = self.op.read(path).await.map_err(io)?.to_vec();
                if data.len() > MAX_READ {
                    return Err(format!("{path} exceeds 256 MiB"));
                }
                data
            }
        };
        Ok(data)
    }

    /// Immediate children of `path`; directories end with `/`.
    pub async fn list(&self, path: &str) -> Result<Vec<String>> {
        let trimmed = path.trim_matches('/');
        if let Some(root) = &self.http_root {
            return self.list_html(root, trimmed).await;
        }
        let dir = if trimmed.is_empty() {
            "/".to_owned()
        } else {
            format!("{trimmed}/")
        };
        let entries = match self.op.list(&dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(io(e)),
        };
        let mut names: Vec<String> = entries
            .iter()
            .filter(|e| e.path().trim_matches('/') != trimmed)
            .map(|e| e.name().to_owned())
            .filter(|n| !n.is_empty() && n != "/")
            .collect();
        names.sort();
        names.dedup();
        Ok(names)
    }

    /// Servers answer `GET dir` with a redirect to the directory index, an HTML page of links.
    async fn list_html(&self, root: &str, dir: &str) -> Result<Vec<String>> {
        if dir.is_empty() {
            return Err("https sources list a directory below the root, such as packs/".to_owned());
        }
        let html = self.read(dir, None).await?;
        if html.len() > 4 * 1024 * 1024 {
            return Err("HTTP directory listing exceeds 4 MiB".to_owned());
        }
        let text = std::str::from_utf8(&html).map_err(|e| e.to_string())?;
        let prefix = format!("{}/{dir}/", root.trim_end_matches('/'));
        Ok(super::extract_html_links(text, &prefix))
    }

    /// Wraps an existing operator, for tests over the in-memory service.
    #[cfg(test)]
    pub fn from_operator(op: Operator) -> Self {
        Self {
            op,
            http_root: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> Object {
        let op = Operator::new(opendal::services::Memory::default()).unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            op.write("data/packs/a.cpk", b"0123456789".to_vec())
                .await
                .unwrap();
            op.write("data/packs/sub/b.cpk", b"bb".to_vec())
                .await
                .unwrap();
            op.write("data/cpk_list.cfg.bin", b"cfg".to_vec())
                .await
                .unwrap();
        });
        Object::from_operator(op)
    }

    fn run<T>(f: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(f)
    }

    #[test]
    fn ranged_reads_return_exactly_the_requested_bytes() {
        let object = memory();
        assert_eq!(
            run(object.read("data/packs/a.cpk", Some((3, 4)))).unwrap(),
            b"3456"
        );
        assert_eq!(
            run(object.read("data/packs/a.cpk", None)).unwrap(),
            b"0123456789"
        );
        let past_end = run(object.read("data/packs/a.cpk", Some((8, 5))));
        assert!(past_end.is_err(), "{past_end:?}");
        assert!(run(object.read("data/missing.bin", None)).is_err());
    }

    #[test]
    fn listing_returns_immediate_children_with_directory_slashes() {
        let object = memory();
        assert_eq!(
            run(object.list("data")).unwrap(),
            ["cpk_list.cfg.bin", "packs/"]
        );
        assert_eq!(run(object.list("data/packs")).unwrap(), ["a.cpk", "sub/"]);
        assert_eq!(run(object.list("data/none")).unwrap(), Vec::<String>::new());
    }
}
