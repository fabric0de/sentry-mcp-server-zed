use serde::Deserialize;
use std::{
    env, fs,
    path::{Component, Path},
};
use zed_extension_api::{
    self as zed, settings::ContextServerSettings, Command, ContextServerConfiguration,
    ContextServerId, Project,
};

const CONTEXT_SERVER_ID: &str = "sentry-mcp";
const NPM_PACKAGE_NAME: &str = "@sentry/mcp-server";
const NPM_PACKAGE_DIR: &str = "node_modules/@sentry/mcp-server";

struct SentryMcpExtension;

#[derive(Deserialize)]
struct NpmPackageManifest {
    bin: NpmBin,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum NpmBin {
    Commands {
        #[serde(rename = "sentry-mcp")]
        sentry_mcp: String,
    },
    Path(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct SentryMcpSettings {
    sentry_access_token: String,
    sentry_host: String,
}

impl Default for SentryMcpSettings {
    fn default() -> Self {
        Self {
            sentry_access_token: String::new(),
            sentry_host: String::new(),
        }
    }
}

impl zed::Extension for SentryMcpExtension {
    fn new() -> Self {
        Self
    }

    fn context_server_command(
        &mut self,
        context_server_id: &ContextServerId,
        project: &Project,
    ) -> zed::Result<Command> {
        if context_server_id.as_ref() != CONTEXT_SERVER_ID {
            return Err(format!(
                "Unsupported context server id `{}` for this extension",
                context_server_id.as_ref()
            ));
        }

        ensure_sentry_mcp_installed()?;
        let settings = load_settings(context_server_id, project)?;
        let env = command_env(&settings)?;
        let cwd = env::current_dir().map_err(|err| {
            format!("Could not resolve current directory for sentry-mcp entrypoint lookup: {err}")
        })?;
        let entrypoint = resolve_sentry_entrypoint(&cwd.join(NPM_PACKAGE_DIR))?;

        Ok(Command {
            command: zed::node_binary_path()?,
            args: vec![entrypoint],
            env,
        })
    }

    fn context_server_configuration(
        &mut self,
        context_server_id: &ContextServerId,
        _project: &Project,
    ) -> zed::Result<Option<ContextServerConfiguration>> {
        if context_server_id.as_ref() != CONTEXT_SERVER_ID {
            return Ok(None);
        }

        Ok(Some(ContextServerConfiguration {
            installation_instructions: include_str!(
                "../configuration/installation_instructions.md"
            )
            .to_string(),
            settings_schema: include_str!("../configuration/settings_schema.json").to_string(),
            default_settings: include_str!("../configuration/default_settings.jsonc").to_string(),
        }))
    }
}

fn ensure_sentry_mcp_installed() -> zed::Result<()> {
    let latest_version = zed::npm_package_latest_version(NPM_PACKAGE_NAME)?;
    let installed_version = zed::npm_package_installed_version(NPM_PACKAGE_NAME)?;

    if installed_version.as_deref() != Some(latest_version.as_str()) {
        zed::npm_install_package(NPM_PACKAGE_NAME, &latest_version)?;
    }
    Ok(())
}

fn resolve_sentry_entrypoint(package_dir: &Path) -> zed::Result<String> {
    let manifest_path = package_dir.join("package.json");
    let manifest = fs::read(&manifest_path)
        .map_err(|err| format!("Could not read {}: {err}", manifest_path.display()))?;
    let manifest: NpmPackageManifest = zed::serde_json::from_slice(&manifest)
        .map_err(|err| format!("Could not parse {}: {err}", manifest_path.display()))?;
    let bin = match &manifest.bin {
        NpmBin::Commands { sentry_mcp } => sentry_mcp,
        NpmBin::Path(path) => path,
    };
    let relative_path = Path::new(bin);
    if bin.is_empty()
        || relative_path
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err(format!(
            "Expected a package-relative executable path in the bin field of {}",
            manifest_path.display()
        ));
    }
    let entrypoint = package_dir.join(relative_path);

    if entrypoint.is_file() {
        Ok(entrypoint.to_string_lossy().to_string())
    } else {
        Err(format!(
            "Could not locate @sentry/mcp-server entrypoint at {}",
            entrypoint.to_string_lossy()
        ))
    }
}

fn load_settings(
    context_server_id: &ContextServerId,
    project: &Project,
) -> zed::Result<SentryMcpSettings> {
    let context_settings = ContextServerSettings::for_project(context_server_id.as_ref(), project)?;
    let settings = if let Some(value) = context_settings.settings {
        zed::serde_json::from_value(value)
            .map_err(|err| format!("Invalid settings for `{CONTEXT_SERVER_ID}`: {err}"))?
    } else {
        SentryMcpSettings::default()
    };

    Ok(settings)
}

fn command_env(settings: &SentryMcpSettings) -> zed::Result<Vec<(String, String)>> {
    let token = settings.sentry_access_token.trim();

    if token.is_empty() {
        return Err(
            "Missing required setting: sentry_access_token. Open Configure for Sentry MCP Server and set `sentry_access_token`."
                .to_string(),
        );
    }

    let mut env = vec![
        ("SENTRY_AUTH_TOKEN".to_string(), token.to_string()),
        // An inherited legacy token would otherwise override the configured one.
        ("SENTRY_ACCESS_TOKEN".to_string(), String::new()),
    ];
    let host = settings.sentry_host.trim();

    if !host.is_empty() {
        env.push(("SENTRY_HOST".to_string(), host.to_string()));
    }
    Ok(env)
}

zed::register_extension!(SentryMcpExtension);

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn resolves_the_installed_packages_bin() {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let package = env::temp_dir().join(format!("sentry-mcp-bin-{}", nonce.as_nanos()));
        fs::create_dir_all(package.join("dist")).unwrap();
        let old_entrypoint = package.join("dist/index.js");
        let new_entrypoint = package.join("dist/sentry-mcp.cjs");
        fs::write(&old_entrypoint, "").unwrap();
        fs::write(&new_entrypoint, "").unwrap();

        let manifest = package.join("package.json");
        fs::write(
            &manifest,
            r#"{"bin":{"sentry-mcp":"./dist/sentry-mcp.cjs"}}"#,
        )
        .unwrap();
        let bundled = resolve_sentry_entrypoint(&package);
        fs::write(&manifest, r#"{"bin":"dist/index.js"}"#).unwrap();
        let single_bin = resolve_sentry_entrypoint(&package);
        fs::write(&manifest, r#"{"bin":{"sentry-mcp":"dist/missing.cjs"}}"#).unwrap();
        let missing = resolve_sentry_entrypoint(&package);
        fs::remove_dir_all(&package).unwrap();

        assert_eq!(Path::new(&bundled.unwrap()), new_entrypoint.as_path());
        assert_eq!(Path::new(&single_bin.unwrap()), old_entrypoint.as_path());
        assert!(missing.unwrap_err().contains("missing.cjs"));
    }

    #[test]
    fn passes_existing_settings_via_the_server_environment() {
        let mut settings = SentryMcpSettings {
            sentry_access_token: " sntryu_test ".to_string(),
            sentry_host: " https://sentry.example.com ".to_string(),
        };
        let token_env = vec![
            ("SENTRY_AUTH_TOKEN".to_string(), "sntryu_test".to_string()),
            ("SENTRY_ACCESS_TOKEN".to_string(), String::new()),
        ];
        let mut self_hosted_env = token_env.clone();
        self_hosted_env.push((
            "SENTRY_HOST".to_string(),
            "https://sentry.example.com".to_string(),
        ));
        assert_eq!(command_env(&settings).unwrap(), self_hosted_env);

        settings.sentry_host.clear();
        assert_eq!(command_env(&settings).unwrap(), token_env);
        assert!(command_env(&SentryMcpSettings::default()).is_err());
    }
}
