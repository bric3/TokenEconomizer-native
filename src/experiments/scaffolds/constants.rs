use super::{Result, Value, json};

pub fn constants() -> Result<Value> {
    let generator = std::env::current_exe()?
        .with_file_name(format!("te-repomap-gen{}", std::env::consts::EXE_SUFFIX));
    Ok(json!({
        "rtk":{
            "RTK_VERSION":"0.45.0",
            "RTK_URL":"https://github.com/rtk-ai/rtk/releases/download/v0.45.0/rtk-x86_64-unknown-linux-musl.tar.gz",
            "RTK_INSTALL_SH":"curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh",
            "class":{"name":"rtk","patch_excludes":["RTK.md",".rtk",".rtk/**"],
                "COPILOT_HOOK_FILE":"rtk-rewrite.json","RTK_RULE":"Always prefix shell commands with",
                "_PROBE_PAYLOAD":"{\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"ls -la\"}}"}},
        "serena":{
            "SERENA_TOOLS":["find_symbol","get_symbols_overview","find_referencing_symbols","find_implementations","search_for_pattern"],
            "INSTALL":"curl -LsSf https://astral.sh/uv/install.sh | sh >/tmp/uv.log 2>&1; export PATH=$HOME/.local/bin:$PATH; uv tool install -p 3.13 serena-agent >/tmp/serena_install.log 2>&1 && serena --version",
            "class":{"name":"serena","patch_excludes":[".serena",".serena/**"]}},
        "repomap":{
            "DEFAULT_MAP_TOKENS":2048,"GEN":generator,
            "HEADER":"# Repository map\nRanked key files and symbols (signatures only) for this repository. Consult this to locate relevant code before searching.\n\n",
            "class":{"name":"repomap","patch_excludes":[],
                "TAR_SCRIPT":"cd /testbed && git ls-files -z -- '*.py' | tar -cf - --null -T - 2>/dev/null"}}
    }))
}
