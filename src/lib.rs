use serde::Deserialize;
use zed_extension_api::{
    self as zed, settings::ContextServerSettings, Command, ContextServerConfiguration,
    ContextServerId, Project,
};

const CONTEXT_SERVER_ID: &str = "sentry-mcp";
const NPM_PACKAGE_NAME: &str = "@sentry/mcp-server";
const NPM_ENTRYPOINT_RELATIVE: &str = "node_modules/@sentry/mcp-server/dist/bin/mcp.js";

struct SentryMcpExtension;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct SentryMcpSettings {
    sentry_access_token: String,
}

impl Default for SentryMcpSettings {
    fn default() -> Self {
        Self {
            sentry_access_token: String::new(),
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

        let token = settings.sentry_access_token.trim();
        if token.is_empty() {
            return Err("Missing required setting: sentry_access_token".to_string());
        }

        let args = vec![
            NPM_ENTRYPOINT_RELATIVE.to_string(),
            format!("--access-token={token}"),
        ];

        Ok(Command {
            command: zed::node_binary_path()?,
            args,
            env: Vec::new(),
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

zed::register_extension!(SentryMcpExtension);
