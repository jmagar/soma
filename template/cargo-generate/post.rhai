let package_name = variable::get("package_name");
let crate_prefix = variable::get("crate_prefix");
let binary_name = variable::get("binary_name");
let server_binary_name = variable::get("server_binary_name");
let service_slug = variable::get("service_slug");
let type_prefix = variable::get("type_prefix");
let env_prefix = variable::get("env_prefix");
let scope_prefix = variable::get("scope_prefix");
let default_port = variable::get("default_port");
let github_owner = variable::get("github_owner");
let github_repo = variable::get("github_repo");
let default_features = variable::get("default_features");
let rewrite_script = env::working_directory + "/template/cargo-generate/rewrite.py";
let generated_root = env::working_directory;

system::command("python3", [
    rewrite_script,
    generated_root,
    package_name,
    crate_prefix,
    binary_name,
    server_binary_name,
    service_slug,
    type_prefix,
    env_prefix,
    scope_prefix,
    default_port,
    github_owner,
    github_repo,
    default_features,
]);
