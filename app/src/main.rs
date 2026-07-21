use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;

const RAW_MODEL_BASE: &str =
    "https://raw.githubusercontent.com/Do-EE2-IoT/ai-ota-manager/main/model/";
const HC_MODEL_BASE: &str = "/etc/smarthome/";
const LISTEN_ADDR: &str = "127.0.0.1:8787";

fn main() -> Result<(), String> {
    let repo_root = find_repo_root();
    let listener =
        TcpListener::bind(LISTEN_ADDR).map_err(|e| format!("Không mở được {LISTEN_ADDR}: {e}"))?;
    println!("AI OTA Manager đang chạy tại http://{LISTEN_ADDR}");
    println!("Repo: {}", repo_root.display());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let root = repo_root.clone();
                if let Err(err) = handle_connection(stream, &root) {
                    eprintln!("{err}");
                }
            }
            Err(err) => eprintln!("Lỗi kết nối: {err}"),
        }
    }
    Ok(())
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct ConfigFile {
    env: Vec<String>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct ManifestModel {
    version: u64,
    url: String,
    path: String,
    sha256: String,
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct ManifestConfig {
    version: u64,
    file: String,
    sha256: String,
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct ManifestFile {
    detect: ManifestModel,
    verify: ManifestModel,
    config: ManifestConfig,
}

#[derive(Clone, Serialize)]
struct HcSummary {
    dir: String,
    mac: String,
}

#[derive(Clone, Serialize)]
struct ModelSummary {
    name: String,
    size: u64,
    used: bool,
}

#[derive(Serialize)]
struct AppState {
    repo_root: String,
    hcs: Vec<HcSummary>,
    models: Vec<ModelSummary>,
    status: String,
}

#[derive(Serialize)]
struct HcDetails {
    dir: String,
    mac: String,
    detect_model: String,
    verify_model: String,
    detect_version: u64,
    verify_version: u64,
    config_version: u64,
    env_text: String,
}

#[derive(Deserialize)]
struct SaveHcRequest {
    mac: String,
    detect_model: String,
    verify_model: String,
    detect_version: u64,
    verify_version: u64,
    config_version: u64,
    env_text: String,
}

#[derive(Deserialize)]
struct DirRequest {
    dir: String,
}

#[derive(Deserialize)]
struct ModelPathRequest {
    path: String,
}

#[derive(Deserialize)]
struct ModelNameRequest {
    name: String,
}

#[derive(Deserialize)]
struct PublishRequest {
    user: String,
    token: String,
    message: String,
}

#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    ok: bool,
    data: Option<T>,
    message: String,
}

fn handle_connection(mut stream: TcpStream, repo_root: &Path) -> Result<(), String> {
    let mut buf = Vec::new();
    let mut tmp = [0; 8192];
    loop {
        let n = stream
            .read(&mut tmp)
            .map_err(|e| format!("Không đọc được request: {e}"))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(header_end) = find_header_end(&buf) {
            let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
            let content_len = content_length(&headers);
            let body_start = header_end + 4;
            while buf.len() < body_start + content_len {
                let n = stream
                    .read(&mut tmp)
                    .map_err(|e| format!("Không đọc được request body: {e}"))?;
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            let body = buf[body_start..buf.len().min(body_start + content_len)].to_vec();
            return route_request(&mut stream, repo_root, &headers, &body);
        }
    }
    Ok(())
}

fn route_request(
    stream: &mut TcpStream,
    repo_root: &Path,
    headers: &str,
    body: &[u8],
) -> Result<(), String> {
    let Some(request_line) = headers.lines().next() else {
        return respond_text(stream, 400, "Bad Request", "Request rỗng");
    };
    let parts = request_line.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 {
        return respond_text(stream, 400, "Bad Request", "Request line không hợp lệ");
    }
    let method = parts[0];
    let path = parts[1].split('?').next().unwrap_or(parts[1]);

    match (method, path) {
        ("GET", "/") => respond_html(stream, HTML),
        ("GET", "/api/state") => respond_json_result(stream, state(repo_root)),
        ("GET", path) if path.starts_with("/api/hc/") => {
            let dir = path.trim_start_matches("/api/hc/");
            respond_json_result(stream, load_hc(repo_root, dir))
        }
        ("POST", "/api/hc/save") => {
            let req: SaveHcRequest = json_body(body)?;
            respond_json_result(stream, save_hc(repo_root, req))
        }
        ("POST", "/api/hc/delete") => {
            let req: DirRequest = json_body(body)?;
            respond_json_result(stream, delete_hc(repo_root, &req.dir))
        }
        ("POST", "/api/model/add") => {
            let req: ModelPathRequest = json_body(body)?;
            respond_json_result(stream, add_model(repo_root, &req.path))
        }
        ("POST", "/api/model/delete") => {
            let req: ModelNameRequest = json_body(body)?;
            respond_json_result(stream, delete_model(repo_root, &req.name))
        }
        ("POST", "/api/validate") => respond_json_result(stream, validate(repo_root)),
        ("POST", "/api/publish") => {
            let req: PublishRequest = json_body(body)?;
            respond_json_result(stream, publish(repo_root, req))
        }
        _ => respond_text(stream, 404, "Not Found", "Không tìm thấy"),
    }
}

fn state(repo_root: &Path) -> Result<AppState, String> {
    let hcs = list_dirs(&repo_root.join("hc"))
        .into_iter()
        .map(|dir| HcSummary {
            mac: dir_to_mac(&dir),
            dir,
        })
        .collect();

    let all_hcs = list_dirs(&repo_root.join("hc"));
    let models = list_files_with_ext(&repo_root.join("model"), "lum")
        .into_iter()
        .map(|name| {
            let size = fs::metadata(repo_root.join("model").join(&name))
                .map(|m| m.len())
                .unwrap_or(0);
            let used = model_is_used(repo_root, &all_hcs, &name);
            ModelSummary { name, size, used }
        })
        .collect();

    Ok(AppState {
        repo_root: repo_root.display().to_string(),
        hcs,
        models,
        status: "OK".to_string(),
    })
}

fn load_hc(repo_root: &Path, dir: &str) -> Result<HcDetails, String> {
    validate_dir_name(dir)?;
    let hc_dir = repo_root.join("hc").join(dir);
    let manifest: ManifestFile = read_json(&hc_dir.join("manifest.json"))?;
    let config: ConfigFile = read_json(&hc_dir.join("config.json"))?;

    Ok(HcDetails {
        dir: dir.to_string(),
        mac: dir_to_mac(dir),
        detect_model: filename_from_url_or_path(&manifest.detect.url, &manifest.detect.path),
        verify_model: filename_from_url_or_path(&manifest.verify.url, &manifest.verify.path),
        detect_version: manifest.detect.version.max(1),
        verify_version: manifest.verify.version.max(1),
        config_version: manifest.config.version.max(1),
        env_text: config.env.join("\n"),
    })
}

fn save_hc(repo_root: &Path, req: SaveHcRequest) -> Result<String, String> {
    let dir = mac_to_dir(&req.mac)?;
    if req.detect_model.trim().is_empty() || req.verify_model.trim().is_empty() {
        return Err("Cần chọn cả model detect và model verify.".to_string());
    }
    ensure_model_name(&req.detect_model)?;
    ensure_model_name(&req.verify_model)?;

    let mut env = parse_env_lines(&req.env_text)?;
    let detect_name = req.detect_model.trim();
    let verify_name = req.verify_model.trim();
    set_env_value(
        &mut env,
        "MODEL_PATH",
        &format!("{HC_MODEL_BASE}{detect_name}"),
    );
    set_env_value(
        &mut env,
        "VERIFY_MODEL_PATH",
        &format!("{HC_MODEL_BASE}{verify_name}"),
    );

    let hc_dir = repo_root.join("hc").join(&dir);
    fs::create_dir_all(&hc_dir).map_err(|e| format!("Không tạo được thư mục HC: {e}"))?;

    write_json(&hc_dir.join("config.json"), &ConfigFile { env })?;
    write_json(
        &hc_dir.join("manifest.json"),
        &ManifestFile {
            detect: ManifestModel {
                version: req.detect_version.max(1),
                url: format!("{RAW_MODEL_BASE}{detect_name}"),
                path: format!("{HC_MODEL_BASE}{detect_name}"),
                sha256: String::new(),
            },
            verify: ManifestModel {
                version: req.verify_version.max(1),
                url: format!("{RAW_MODEL_BASE}{verify_name}"),
                path: format!("{HC_MODEL_BASE}{verify_name}"),
                sha256: String::new(),
            },
            config: ManifestConfig {
                version: req.config_version.max(1),
                file: "config.json".to_string(),
                sha256: String::new(),
            },
        },
    )?;

    run_checked(
        Command::new("python3")
            .arg("tools/refresh-manifest-sha.py")
            .arg(format!("hc/{dir}"))
            .current_dir(repo_root),
    )?;
    validate(repo_root)?;
    Ok(format!("Đã lưu và kiểm tra thành công HC {dir}"))
}

fn delete_hc(repo_root: &Path, dir: &str) -> Result<String, String> {
    validate_dir_name(dir)?;
    fs::remove_dir_all(repo_root.join("hc").join(dir))
        .map_err(|e| format!("Không xóa được HC {dir}: {e}"))?;
    Ok(format!("Đã xóa HC {dir}"))
}

fn add_model(repo_root: &Path, raw_path: &str) -> Result<String, String> {
    let src = PathBuf::from(raw_path.trim());
    if src.extension().and_then(|s| s.to_str()) != Some("lum") {
        return Err("Chỉ hỗ trợ file model có đuôi .lum.".to_string());
    }
    let name = src
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Tên file model không hợp lệ.".to_string())?;
    ensure_model_name(name)?;
    fs::copy(&src, repo_root.join("model").join(name))
        .map_err(|e| format!("Không copy được model: {e}"))?;
    Ok(format!("Đã thêm model {name}"))
}

fn delete_model(repo_root: &Path, name: &str) -> Result<String, String> {
    ensure_model_name(name)?;
    let hcs = list_dirs(&repo_root.join("hc"));
    if model_is_used(repo_root, &hcs, name) {
        return Err(format!("Model {name} đang được HC sử dụng, không xóa."));
    }
    fs::remove_file(repo_root.join("model").join(name))
        .map_err(|e| format!("Không xóa được model {name}: {e}"))?;
    Ok(format!("Đã xóa model {name}"))
}

fn validate(repo_root: &Path) -> Result<String, String> {
    let mut output = String::new();
    for hc in list_dirs(&repo_root.join("hc")) {
        run_collect(
            Command::new("python3")
                .arg("-m")
                .arg("json.tool")
                .arg(format!("hc/{hc}/config.json"))
                .current_dir(repo_root),
            &mut output,
        )?;
        run_collect(
            Command::new("python3")
                .arg("-m")
                .arg("json.tool")
                .arg(format!("hc/{hc}/manifest.json"))
                .current_dir(repo_root),
            &mut output,
        )?;
    }
    run_collect(
        Command::new("sh")
            .arg("-n")
            .arg("scripts/check-model-update.sh")
            .current_dir(repo_root),
        &mut output,
    )?;
    run_collect(
        Command::new("sh")
            .arg("-n")
            .arg("scripts/ota-lib.sh")
            .current_dir(repo_root),
        &mut output,
    )?;

    if output.trim().is_empty() {
        Ok("Kiểm tra OK".to_string())
    } else {
        Ok(output)
    }
}

fn publish(repo_root: &Path, req: PublishRequest) -> Result<String, String> {
    if req.user.trim().is_empty() || req.token.trim().is_empty() {
        return Err("Cần nhập Git username và token.".to_string());
    }
    if req.message.trim().is_empty() {
        return Err("Cần nhập commit message.".to_string());
    }
    validate(repo_root)?;

    let askpass = repo_root.join(".git").join("ota-manager-askpass.sh");
    let script = format!(
        "#!/bin/sh\ncase \"$1\" in\n*Username*) printf '%s\\n' '{}';;\n*) printf '%s\\n' '{}';;\nesac\n",
        shell_single_quote(&req.user),
        shell_single_quote(&req.token)
    );
    fs::write(&askpass, script).map_err(|e| format!("Không tạo được askpass: {e}"))?;
    set_executable(&askpass)?;

    let result = (|| {
        run_git_with_auth(
            repo_root,
            &askpass,
            ["add", "README.md", "app", "hc", "model"],
        )?;
        let status = run_output(
            Command::new("git")
                .arg("status")
                .arg("--porcelain")
                .current_dir(repo_root),
        )?;
        if status.trim().is_empty() {
            return Ok("Không có thay đổi để xuất bản.".to_string());
        }
        run_git_with_auth(repo_root, &askpass, ["commit", "-m", req.message.trim()])?;
        run_git_with_auth(repo_root, &askpass, ["pull", "--rebase", "origin", "main"])?;
        run_git_with_auth(repo_root, &askpass, ["push", "origin", "main"])?;
        Ok("Xuất bản thành công lên origin/main.".to_string())
    })();

    let _ = fs::remove_file(&askpass);
    result
}

fn respond_json_result<T: Serialize>(
    stream: &mut TcpStream,
    result: Result<T, String>,
) -> Result<(), String> {
    match result {
        Ok(data) => respond_json(
            stream,
            200,
            &ApiResponse {
                ok: true,
                data: Some(data),
                message: String::new(),
            },
        ),
        Err(message) => respond_json(
            stream,
            200,
            &ApiResponse::<()> {
                ok: false,
                data: None,
                message,
            },
        ),
    }
}

fn respond_json<T: Serialize>(
    stream: &mut TcpStream,
    status: u16,
    value: &T,
) -> Result<(), String> {
    let body =
        serde_json::to_string(value).map_err(|e| format!("Không serialize response: {e}"))?;
    respond(
        stream,
        status,
        "OK",
        "application/json; charset=utf-8",
        &body,
    )
}

fn respond_html(stream: &mut TcpStream, body: &str) -> Result<(), String> {
    respond(stream, 200, "OK", "text/html; charset=utf-8", body)
}

fn respond_text(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    body: &str,
) -> Result<(), String> {
    respond(stream, status, reason, "text/plain; charset=utf-8", body)
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &str,
) -> Result<(), String> {
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|e| format!("Không ghi được response: {e}"))
}

fn json_body<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|e| format!("JSON request không hợp lệ: {e}"))
}

fn content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0)
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn list_dirs(path: &Path) -> Vec<String> {
    let mut items = fs::read_dir(path)
        .ok()
        .into_iter()
        .flat_map(|it| it.filter_map(Result::ok))
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().to_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    items.sort();
    items
}

fn list_files_with_ext(path: &Path, ext: &str) -> Vec<String> {
    let mut items = fs::read_dir(path)
        .ok()
        .into_iter()
        .flat_map(|it| it.filter_map(Result::ok))
        .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some(ext))
        .filter_map(|entry| entry.file_name().to_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    items.sort();
    items
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("Không đọc được {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("JSON lỗi {}: {e}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Không serialize được {}: {e}", path.display()))?;
    text.push('\n');
    fs::write(path, text).map_err(|e| format!("Không ghi được {}: {e}", path.display()))
}

fn mac_to_dir(mac: &str) -> Result<String, String> {
    let clean = mac.trim().to_lowercase().replace('-', ":");
    let parts = clean.split(':').collect::<Vec<_>>();
    if parts.len() != 6
        || parts
            .iter()
            .any(|p| p.len() != 2 || !p.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err("MAC không hợp lệ. Ví dụ: 24:95:07:e0:81:96".to_string());
    }
    Ok(parts.join("-"))
}

fn validate_dir_name(dir: &str) -> Result<(), String> {
    if dir.len() == 17
        && dir
            .split('-')
            .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
    {
        Ok(())
    } else {
        Err("Tên thư mục HC không hợp lệ.".to_string())
    }
}

fn ensure_model_name(name: &str) -> Result<(), String> {
    if name.ends_with(".lum") && !name.contains('/') && !name.contains('\\') && !name.is_empty() {
        Ok(())
    } else {
        Err("Tên model không hợp lệ, phải là file .lum trong thư mục model/.".to_string())
    }
}

fn dir_to_mac(dir: &str) -> String {
    dir.replace('-', ":")
}

fn filename_from_url_or_path(url: &str, path: &str) -> String {
    url.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(""))
        .to_string()
}

fn parse_env_lines(text: &str) -> Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    let mut env = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, _value)) = line.split_once('=') else {
            return Err(format!("Dòng env không đúng KEY=VALUE: {line}"));
        };
        if key.trim().is_empty() || key.chars().any(char::is_whitespace) {
            return Err(format!("Key env không hợp lệ: {key}"));
        }
        if !seen.insert(key.to_string()) {
            return Err(format!("Key env bị lặp: {key}"));
        }
        env.push(line.to_string());
    }
    if env.is_empty() {
        return Err("Config env không được rỗng.".to_string());
    }
    Ok(env)
}

fn set_env_value(env: &mut Vec<String>, key: &str, value: &str) {
    let assignment = format!("{key}={value}");
    for item in env.iter_mut() {
        if item.split_once('=').map(|(k, _)| k) == Some(key) {
            *item = assignment;
            return;
        }
    }
    env.push(assignment);
}

fn model_is_used(repo_root: &Path, hcs: &[String], name: &str) -> bool {
    for dir in hcs {
        let manifest_path = repo_root.join("hc").join(dir).join("manifest.json");
        if let Ok(manifest) = read_json::<ManifestFile>(&manifest_path) {
            if filename_from_url_or_path(&manifest.detect.url, &manifest.detect.path) == name
                || filename_from_url_or_path(&manifest.verify.url, &manifest.verify.path) == name
            {
                return true;
            }
        }
    }
    false
}

fn find_repo_root() -> PathBuf {
    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.to_path_buf());
        }
    }

    for start in candidates {
        for path in start.ancestors() {
            if path.join("hc").is_dir()
                && path.join("model").is_dir()
                && path.join("tools").is_dir()
            {
                return path.to_path_buf();
            }
        }
    }

    PathBuf::from("..")
}

fn run_checked(cmd: &mut Command) -> Result<(), String> {
    let output = cmd
        .output()
        .map_err(|e| format!("Không chạy được command: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(command_error(&output))
    }
}

fn run_collect(cmd: &mut Command, out: &mut String) -> Result<(), String> {
    let output = cmd
        .output()
        .map_err(|e| format!("Không chạy được command: {e}"))?;
    out.push_str(&String::from_utf8_lossy(&output.stdout));
    out.push_str(&String::from_utf8_lossy(&output.stderr));
    if output.status.success() {
        Ok(())
    } else {
        Err(command_error(&output))
    }
}

fn run_output(cmd: &mut Command) -> Result<String, String> {
    let output = cmd
        .output()
        .map_err(|e| format!("Không chạy được command: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(command_error(&output))
    }
}

fn run_git_with_auth<const N: usize>(
    repo_root: &Path,
    askpass: &Path,
    args: [&str; N],
) -> Result<(), String> {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(repo_root)
        .env("GIT_ASKPASS", askpass)
        .env("GIT_TERMINAL_PROMPT", "0");
    run_checked(&mut cmd)
}

fn command_error(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!(
        "Command lỗi code={:?}\n{}{}",
        output.status.code(),
        stdout,
        stderr
    )
}

fn shell_single_quote(value: &str) -> String {
    value.replace('\'', "'\\''")
}

fn set_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)
            .map_err(|e| format!("Không đọc được quyền {}: {e}", path.display()))?
            .permissions();
        perms.set_mode(0o700);
        fs::set_permissions(path, perms)
            .map_err(|e| format!("Không set được quyền {}: {e}", path.display()))?;
    }
    Ok(())
}

const HTML: &str = r#"<!doctype html>
<html lang="vi">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>AI OTA Manager</title>
  <style>
    :root { color-scheme: light; font-family: Inter, Roboto, "Segoe UI", Arial, sans-serif; color: #17212b; background: #eef2f5; }
    * { box-sizing: border-box; }
    body { margin: 0; font-size: 16px; }
    header { height: 58px; display: flex; align-items: center; gap: 18px; padding: 0 22px; background: #ffffff; border-bottom: 1px solid #d8dee5; }
    header h1 { margin: 0; font-size: 21px; font-weight: 700; }
    nav button, button { border: 1px solid #c8d1dc; background: #fff; color: #17212b; border-radius: 6px; padding: 9px 13px; font-size: 15px; cursor: pointer; }
    nav button.active, button.primary { background: #1769aa; border-color: #1769aa; color: white; }
    button.danger { color: #a12620; border-color: #e0b8b4; }
    main { display: grid; grid-template-columns: 300px 1fr; gap: 18px; padding: 18px; min-height: calc(100vh - 98px); }
    aside, section, .panel { background: white; border: 1px solid #d8dee5; border-radius: 8px; padding: 16px; }
    h2 { margin: 0 0 14px; font-size: 20px; }
    h3 { margin: 0 0 10px; font-size: 17px; }
    .list { display: grid; gap: 8px; }
    .item { width: 100%; text-align: left; display: flex; justify-content: space-between; gap: 8px; }
    .item.active { border-color: #1769aa; box-shadow: 0 0 0 1px #1769aa inset; }
    .form { display: grid; gap: 13px; }
    .row { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
    label { display: grid; gap: 6px; font-weight: 600; }
    input, select, textarea { width: 100%; border: 1px solid #c8d1dc; border-radius: 6px; padding: 9px 10px; font: inherit; background: white; color: #17212b; }
    textarea { min-height: 310px; resize: vertical; font-family: "Ubuntu Mono", "DejaVu Sans Mono", Consolas, monospace; font-size: 15px; line-height: 1.45; }
    .urlbox { display: grid; gap: 6px; padding: 10px 12px; background: #f5f7f9; border-radius: 6px; border: 1px solid #e1e6eb; overflow-wrap: anywhere; }
    .actions { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; }
    .muted { color: #647282; font-size: 14px; }
    .status { position: sticky; bottom: 0; padding: 10px 18px; background: #17212b; color: white; min-height: 40px; }
    .hidden { display: none; }
    table { width: 100%; border-collapse: collapse; }
    th, td { padding: 10px; border-bottom: 1px solid #e5e9ee; text-align: left; }
    pre { white-space: pre-wrap; background: #0f1720; color: #e8edf3; padding: 12px; border-radius: 6px; min-height: 180px; overflow: auto; }
    @media (max-width: 900px) { main { grid-template-columns: 1fr; } .row { grid-template-columns: 1fr; } }
  </style>
</head>
<body>
  <header>
    <h1>AI OTA Manager</h1>
    <nav>
      <button id="tab-hc" class="active" onclick="showTab('hc')">HC</button>
      <button id="tab-model" onclick="showTab('model')">Model</button>
      <button id="tab-publish" onclick="showTab('publish')">Xuất bản</button>
    </nav>
    <span id="repo" class="muted"></span>
  </header>

  <main id="view-hc">
    <aside>
      <div class="actions">
        <h2>Danh sách HC</h2>
        <button class="primary" onclick="newHc()">Thêm HC</button>
      </div>
      <div id="hc-list" class="list"></div>
    </aside>
    <section>
      <h2>Sửa HC</h2>
      <div class="form">
        <div class="row">
          <label>MAC<input id="mac" placeholder="24:95:07:e0:81:96"></label>
          <label>Version detect<input id="detect-version" type="number" min="1" value="1"></label>
          <label>Version verify<input id="verify-version" type="number" min="1" value="1"></label>
        </div>
        <div class="row">
          <label>Version config<input id="config-version" type="number" min="1" value="1"></label>
          <label>Model detect<select id="detect-model" onchange="renderUrls()"></select></label>
          <label>Model verify<select id="verify-model" onchange="renderUrls()"></select></label>
        </div>
        <div class="urlbox">
          <div id="detect-url"></div>
          <div id="detect-path"></div>
          <div id="verify-url"></div>
          <div id="verify-path"></div>
        </div>
        <label>Cấu hình env<textarea id="env-text"></textarea></label>
        <div class="actions">
          <button class="primary" onclick="saveHc()">Lưu</button>
          <button class="danger" onclick="deleteHc()">Xóa HC</button>
        </div>
      </div>
    </section>
  </main>

  <main id="view-model" class="hidden">
    <section style="grid-column: 1 / -1">
      <h2>Danh sách model</h2>
      <div class="actions">
        <input id="model-path" placeholder="/home/user/model-moi.lum">
        <button class="primary" onclick="addModel()">Thêm từ đường dẫn</button>
        <button onclick="loadState()">Tải lại</button>
      </div>
      <p class="muted">App sẽ copy file .lum vào thư mục model/ của repo. Nếu model đang được HC sử dụng thì app sẽ không cho xóa.</p>
      <table>
        <thead><tr><th>Tên file</th><th>Dung lượng</th><th>Trạng thái</th><th></th></tr></thead>
        <tbody id="model-list"></tbody>
      </table>
    </section>
  </main>

  <main id="view-publish" class="hidden">
    <section style="grid-column: 1 / -1">
      <h2>Kiểm tra và xuất bản</h2>
      <div class="form">
        <div class="row">
          <label>Git username<input id="git-user"></label>
          <label>Git token<input id="git-token" type="password"></label>
          <label>Commit message<input id="commit-message" value="Update AI OTA data"></label>
        </div>
        <div class="actions">
          <button onclick="validateRepo()">Kiểm tra</button>
          <button class="primary" onclick="publishRepo()">Xuất bản</button>
        </div>
        <p class="muted">Xuất bản sẽ chạy: git add, git commit, git pull --rebase, git push origin main.</p>
        <pre id="validate-output"></pre>
      </div>
    </section>
  </main>

  <div id="status" class="status">Đang tải...</div>

  <script>
    const RAW = "https://raw.githubusercontent.com/Do-EE2-IoT/ai-ota-manager/main/model/";
    const HC = "/etc/smarthome/";
    let state = { hcs: [], models: [] };
    let currentDir = "";

    function $(id) { return document.getElementById(id); }
    function setStatus(text) { $("status").textContent = text || ""; }
    function modelNames() { return state.models.map(m => m.name); }
    function esc(s) { return String(s ?? "").replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c])); }
    function bytes(n) { return (n / 1024 / 1024).toFixed(2) + " MB"; }

    async function api(path, body) {
      const opt = body === undefined ? {} : { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify(body) };
      const res = await fetch(path, opt);
      const json = await res.json();
      if (!json.ok) throw new Error(json.message);
      return json.data;
    }

    function showTab(tab) {
      for (const name of ["hc", "model", "publish"]) {
        $("view-" + name).classList.toggle("hidden", name !== tab);
        $("tab-" + name).classList.toggle("active", name === tab);
      }
    }

    async function loadState() {
      try {
        state = await api("/api/state");
        $("repo").textContent = state.repo_root;
        renderHcs();
        renderModels();
        renderModelSelects();
        renderUrls();
        setStatus("Sẵn sàng");
      } catch (e) { setStatus(e.message); }
    }

    function renderHcs() {
      $("hc-list").innerHTML = state.hcs.map(h => `<button class="item ${h.dir === currentDir ? "active" : ""}" onclick="loadHc('${h.dir}')"><span>${esc(h.mac)}</span><span class="muted">${esc(h.dir)}</span></button>`).join("");
    }

    function renderModels() {
      $("model-list").innerHTML = state.models.map(m => `<tr><td><code>${esc(m.name)}</code></td><td>${bytes(m.size)}</td><td>${m.used ? "Đang dùng" : "Chưa dùng"}</td><td><button class="danger" onclick="deleteModel('${esc(m.name)}')">Xóa</button></td></tr>`).join("");
    }

    function renderModelSelects() {
      for (const id of ["detect-model", "verify-model"]) {
        const current = $(id).value;
        $(id).innerHTML = modelNames().map(name => `<option value="${esc(name)}">${esc(name)}</option>`).join("");
        if (modelNames().includes(current)) $(id).value = current;
      }
    }

    function renderUrls() {
      const detect = $("detect-model").value || "";
      const verify = $("verify-model").value || "";
      $("detect-url").textContent = "Detect URL: " + RAW + detect;
      $("detect-path").textContent = "Detect path: " + HC + detect;
      $("verify-url").textContent = "Verify URL: " + RAW + verify;
      $("verify-path").textContent = "Verify path: " + HC + verify;
    }

    async function loadHc(dir) {
      try {
        const hc = await api("/api/hc/" + dir);
        currentDir = dir;
        $("mac").value = hc.mac;
        $("detect-version").value = hc.detect_version;
        $("verify-version").value = hc.verify_version;
        $("config-version").value = hc.config_version;
        $("env-text").value = hc.env_text;
        renderModelSelects();
        $("detect-model").value = hc.detect_model;
        $("verify-model").value = hc.verify_model;
        renderUrls();
        renderHcs();
        setStatus("Đã tải HC " + dir);
      } catch (e) { setStatus(e.message); }
    }

    function newHc() {
      currentDir = "";
      $("mac").value = "";
      $("detect-version").value = 1;
      $("verify-version").value = 1;
      $("config-version").value = 1;
      const names = modelNames();
      $("detect-model").value = names[0] || "";
      $("verify-model").value = names[1] || names[0] || "";
      $("env-text").value = defaultEnv($("detect-model").value, $("verify-model").value);
      renderUrls();
      renderHcs();
      setStatus("Nhập MAC mới, chọn model, sửa config rồi bấm Lưu.");
    }

    function defaultEnv(detect, verify) {
      return [
        "DETECT_MAIN_STREAM=false",
        "DETECT_TARGET_FPS=5",
        "IOU_RATE=0.8",
        "LD_LIBRARY_PATH=/usr/local/lib/",
        "MODEL_PATH=" + HC + detect,
        "RECORD_MAIN_STREAM=true",
        "SCORE_BASE=0.7",
        "SCORE_SECURITY=0.9",
        "SCORE_VERIFY=0.5",
        "SECURITY_TIME_WINDOW=5",
        "VERIFY_MODEL_PATH=" + HC + verify,
        "VLM_ENDPOINT=https://api-vlm-gateway.bizfly.cluster.lumi.biz/v1/runtime/resource/verify-cloud"
      ].join("\n");
    }

    async function saveHc() {
      try {
        const msg = await api("/api/hc/save", {
          mac: $("mac").value,
          detect_model: $("detect-model").value,
          verify_model: $("verify-model").value,
          detect_version: Number($("detect-version").value || 1),
          verify_version: Number($("verify-version").value || 1),
          config_version: Number($("config-version").value || 1),
          env_text: $("env-text").value
        });
        await loadState();
        setStatus(msg);
      } catch (e) { setStatus(e.message); }
    }

    async function deleteHc() {
      if (!currentDir || !confirm("Xóa HC " + currentDir + "?")) return;
      try {
        const msg = await api("/api/hc/delete", { dir: currentDir });
        currentDir = "";
        await loadState();
        newHc();
        setStatus(msg);
      } catch (e) { setStatus(e.message); }
    }

    async function addModel() {
      try {
        const msg = await api("/api/model/add", { path: $("model-path").value });
        $("model-path").value = "";
        await loadState();
        setStatus(msg);
      } catch (e) { setStatus(e.message); }
    }

    async function deleteModel(name) {
      if (!confirm("Xóa model " + name + "?")) return;
      try {
        const msg = await api("/api/model/delete", { name });
        await loadState();
        setStatus(msg);
      } catch (e) { setStatus(e.message); }
    }

    async function validateRepo() {
      try {
        const msg = await api("/api/validate", {});
        $("validate-output").textContent = msg;
        setStatus("Kiểm tra OK");
      } catch (e) {
        $("validate-output").textContent = e.message;
        setStatus(e.message);
      }
    }

    async function publishRepo() {
      if (!confirm("Xác nhận xuất bản lên GitHub?")) return;
      try {
        const msg = await api("/api/publish", {
          user: $("git-user").value,
          token: $("git-token").value,
          message: $("commit-message").value
        });
        $("validate-output").textContent = msg;
        setStatus(msg);
      } catch (e) {
        $("validate-output").textContent = e.message;
        setStatus(e.message);
      }
    }

    loadState().then(() => { if (state.hcs[0]) loadHc(state.hcs[0].dir); else newHc(); });
  </script>
</body>
</html>
"#;
