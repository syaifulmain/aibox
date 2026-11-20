use crate::common;
use aibox_core::*;
use serde_json::{json, Value};

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
    match fn_name {
        "Backend::createEnvironmentScript" => {
            json!(provision::create::create_environment_script(
                args["name"].as_str().expect("name"),
                args["localAlias"].as_str().expect("localAlias"),
                args["remoteImage"].as_str().expect("remoteImage"),
                args["workspace"].as_str().expect("workspace"),
                args["cacheBase"].as_str().expect("cacheBase"),
                args["profileFlags"].as_str().expect("profileFlags"),
                args["ramGiB"].as_str().expect("ramGiB"),
                args["diskGiB"].as_str().expect("diskGiB"),
                args["cpuCores"].as_i64().expect("cpuCores") as i32,
                args["x11"].as_bool().expect("x11"),
                args["processLimit"].as_str().expect("processLimit"),
                args["ioLimit"].as_str().expect("ioLimit"),
                args["swapLimit"].as_str().expect("swapLimit"),
                args["isVm"].as_bool().expect("isVm"),
            ))
        }
        "Backend::cloneEnvironmentScript" => {
            let cidrs: Vec<String> = args["resolvedCidrs"]
                .as_array()
                .expect("resolvedCidrs")
                .iter()
                .map(|v| v.as_str().expect("cidr").to_string())
                .collect();
            json!(provision::clone::clone_environment_script(
                args["sourceName"].as_str().expect("sourceName"),
                args["newName"].as_str().expect("newName"),
                args["workspace"].as_str().expect("workspace"),
                args["diskGiB"].as_str().expect("diskGiB"),
                args["ramGiB"].as_str().expect("ramGiB"),
                args["cpuCores"].as_i64().expect("cpuCores") as i32,
                args["startAfter"].as_bool().expect("startAfter"),
                args["netMode"].as_str().expect("netMode"),
                args["netAllowlist"].as_str().expect("netAllowlist"),
                &cidrs,
                args["processLimit"].as_str().expect("processLimit"),
                args["ioLimit"].as_str().expect("ioLimit"),
                args["swapLimit"].as_str().expect("swapLimit"),
                args["isVm"].as_bool().expect("isVm"),
            ))
        }
        "Backend::updateResourcesScript" => {
            json!(provision::resources::update_resources_script(
                args["name"].as_str().expect("name"),
                args["cpuCores"].as_i64().expect("cpuCores") as i32,
                args["ramGiB"].as_str().expect("ramGiB"),
                args["diskGiB"].as_str().expect("diskGiB"),
                args["processLimit"].as_str().expect("processLimit"),
                args["ioLimit"].as_str().expect("ioLimit"),
                args["swapLimit"].as_str().expect("swapLimit"),
                args["isVm"].as_bool().expect("isVm"),
            ))
        }
        "Backend::buildPrepareScript" => {
            json!(provision::lifecycle::build_prepare_script(
                args["name"].as_str().expect("name"),
                "",
                args["allowRefresh"].as_str().expect("allowRefresh"),
            ))
        }
        "Backend::startEnvironmentScript" => {
            json!(provision::lifecycle::start_environment_script(
                args["name"].as_str().expect("name"),
                "",
                "",
            ))
        }
        "Backend::stopEnvironmentScript" => {
            json!(provision::lifecycle::stop_environment_script(
                args["name"].as_str().expect("name"),
                args["includeAutoSnapshot"]
                    .as_bool()
                    .expect("includeAutoSnapshot"),
                "",
            ))
        }
        "Backend::enterIncusExecArgs" => {
            json!(provision::lifecycle::enter_incus_exec_args(
                args["name"].as_str().expect("name"),
                args["remoteEnvPath"].as_str().expect("remoteEnvPath"),
                util::paths::CONTAINER_WORKSPACE_PATH,
                args["display"].as_str().expect("display"),
                args["term"].as_str().expect("term"),
            ))
        }
        "Backend::execIncusCommandArgs" => {
            let cmd: Vec<String> = args["cmd"]
                .as_array()
                .expect("cmd")
                .iter()
                .map(|v| v.as_str().expect("arg").to_string())
                .collect();
            json!(provision::lifecycle::exec_incus_command_args(
                args["name"].as_str().expect("name"),
                &cmd,
                args["remoteEnvPath"].as_str().expect("remoteEnvPath"),
                util::paths::CONTAINER_WORKSPACE_PATH,
            ))
        }
        "Backend::buildEnterCommand" => {
            json!(provision::lifecycle::build_enter_command(
                args["name"].as_str().expect("name"),
                args["remoteEnvPath"].as_str().expect("remoteEnvPath"),
            ))
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    common::replay_jsonl("provision", dispatch);
}
