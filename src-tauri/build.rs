// Tauri build script. Runs `tauri-build` codegen, which reads tauri.conf.json,
// embeds the frontend assets / dev config, and generates the context consumed
// by `tauri::generate_context!()` in src/lib.rs.
fn main() {
    // The official installer build can bake in the project's ClubLog API key via
    // the CLUBLOG_API_KEY env var (read by `option_env!` in src/lib.rs). Cargo does
    // NOT recompile on an env var changing unless told to — without this directive
    // an incremental build could ship a stale/empty key.
    println!("cargo:rerun-if-env-changed=CLUBLOG_API_KEY");

    // AI-CW model presence gate. The DeepCW model is gitignored (AGPL-3.0
    // © e04, 15 MB — see resources/deepcw/README.md), so a fresh checkout or
    // a git WORKTREE builds a tree where resources/deepcw/ has no model, the
    // bundle glob matches nothing, and the result is a fully-green build that
    // ships without the AI CW decoder. That exact artifact reached users on
    // 2026-07-20; release CI now stages+verifies the model, but LOCAL release
    // builds had no guard — this is it. Debug builds only warn (UI work needs
    // no model); release+radio builds fail hard unless explicitly allowed
    // (CI's windows-cross link-check sets NEXUS_ALLOW_MISSING_AICW=1).
    println!("cargo:rerun-if-changed=resources/deepcw");
    println!("cargo:rerun-if-env-changed=NEXUS_ALLOW_MISSING_AICW");
    let radio = std::env::var("CARGO_FEATURE_RADIO").is_ok();
    let model_present = std::path::Path::new("resources/deepcw/model.onnx").exists();
    if radio && !model_present {
        let release = std::env::var("PROFILE").as_deref() == Ok("release");
        let allowed = std::env::var("NEXUS_ALLOW_MISSING_AICW").is_ok();
        if release && !allowed {
            panic!(
                "\nresources/deepcw/model.onnx is MISSING — this release build would ship \
                 with NO AI CW decoder (the silent-lobotomy class: worktrees and fresh \
                 checkouts don't have the gitignored model).\n\
                 Stage it first (see src-tauri/resources/deepcw/README.md), or set \
                 NEXUS_ALLOW_MISSING_AICW=1 to build a knowingly model-less binary.\n"
            );
        }
        println!("cargo:warning=AI CW model missing — this build has no AI CW decoder");
    }

    // Fork/branch/commit stamp. Nothing in the tree identified WHICH build an
    // artifact came from: a .app carries tauri.conf.json's product version and
    // nothing else, so two builds of different branches at the same version are
    // indistinguishable on disk. Identifying a running binary on 2026-08-13 took
    // a hunt for a string literal one commit had added and its parent had not —
    // this is so the build can name itself instead.
    //
    // A source tarball has no git and that is NOT a build failure; the stamp
    // degrades to "unknown".
    println!("cargo:rustc-env=NEXUS_BUILD_ID={}", build_stamp());

    tauri_build::build();
}

/// `owner/repo branch@sha[-dirty]`, or `unknown` where git cannot answer.
fn build_stamp() -> String {
    let git = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git").args(args).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
        (!s.is_empty()).then_some(s)
    };

    // Cargo does not re-run a build script when HEAD moves, so declare it as an
    // input. `--git-path` resolves correctly inside a git WORKTREE, where .git
    // is a FILE rather than a directory and a naive "../.git/HEAD" is wrong.
    if let Some(head) = git(&["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={head}");
    }

    let Some(sha) = git(&["rev-parse", "--short", "HEAD"]) else {
        return "unknown".to_string();
    };
    let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "detached".into());
    // Name the fork too: "macos-support" alone does not say WHOSE macos-support,
    // and the whole point is telling a fork build apart from an upstream one.
    let fork = git(&["remote", "get-url", "origin"])
        .and_then(|u| {
            let u = u.trim_end_matches(".git").trim_end_matches('/');
            let tail: Vec<&str> = u.rsplit(['/', ':']).take(2).collect();
            (tail.len() == 2).then(|| format!("{}/{}", tail[1], tail[0]))
        })
        .unwrap_or_else(|| "local".into());
    let dirty = git(&["status", "--porcelain"]).is_some_and(|s| !s.is_empty());

    format!(
        "{fork} {branch}@{sha}{}",
        if dirty { "-dirty" } else { "" }
    )
}
