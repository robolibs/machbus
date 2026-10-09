-- machbus's build, as recipes. This replaced the Makefile; there is no other.
--
--   make            the recipes, with what each of them says it does
--   make build      the library and CLI
--   make test       the suite
--   make verify     the whole local hardening gate
--
-- At an oslo prompt in this directory `make` is enough; everywhere else it is `oslo make`.
-- The dev shell's toolchain comes from `.env.lua`'s `nix_develop()`, so recipes call `cargo`
-- directly rather than wrapping every command in `nix develop -c`.

local make = oslo.make

local GENERATE_C_ABI_PUBLIC_HEADER = "tools/generate_c_abi_public_header.sh"
local NO_STD_TARGET = "thumbv7em-none-eabihf"
-- Every optional feature the hosted gates must cover. `tim-auth` was outside all
-- of them, so 1200 lines of certificate-chain validation were never type-checked,
-- linted or run by any command a developer or CI invokes.
local HOSTED_FEATURES = "async,tim-auth"

local function need(tool, why)
  assert(oslo.run{ "sh", "-c", "command -v " .. tool, capture = true }.ok, why)
end

-- name = ... from Cargo.toml — the one place every tool reads it from.
local function project_name()
  local content = oslo.fs.read("Cargo.toml") or ""
  local name = content:match('\nname%s*=%s*"([^"]+)"') or content:match('^name%s*=%s*"([^"]+)"')
  assert(name, "Cargo.toml package name not found or invalid")
  return name
end

local NAME = project_name()
local TOP_DIR = oslo.sys.pwd()

make.recipe{
  name = "build",
  desc = "the library and CLI",
  run = function()
    sh.cargo("build", "--lib")
    sh.cargo("build", "-p", "machbus-cli")
  end,
}
make.alias("b", "build")

make.recipe{ name = "compile", desc = "clean, then build", deps = { "clean", "build" } }
make.alias("c", "compile")

make.recipe{
  name = "run",
  desc = "run a development example (if examples exist)",
  params = { { "--example", desc = "which example to run", default = "main" } },
  run = function(a) sh.cargo("run", "--example", a.example or "main") end,
}
make.alias("r", "run")

make.recipe{ name = "test", desc = "run all tests",
             run = function() sh.cargo("test", "--all-targets") end }
make.alias("t", "test")

make.recipe{ name = "check", desc = "cargo check on all targets",
             run = function() sh.cargo("check", "--all-targets") end }

make.recipe{ name = "no-std-check", desc = "check the transitional no_std + alloc embedded surface",
             run = function() sh.cargo("rustc", "--lib", "--no-default-features", "--features", "embedded", "--crate-type", "rlib") end }

make.recipe{
  name = "no-std-target-check",
  desc = ("check no_std on --target (default %s)"):format(NO_STD_TARGET),
  params = { { "--target", desc = "no_std target", default = NO_STD_TARGET } },
  run = function(a)
    local target = a.target or NO_STD_TARGET
    local installed = oslo.run{ "sh", "-c", "rustup target list --installed | grep -qx " .. target }
    assert(installed.ok, "Rust target " .. target .. " is not installed; run: rustup target add " .. target)
    local rustc = oslo.run{ "rustup", "which", "rustc", capture = true }
    assert(rustc.ok, "rustup which rustc failed")
    assert(oslo.run{
      "env", "RUSTC=" .. (rustc.out or ""):gsub("%s+$", ""),
      "cargo", "rustc", "--lib", "--no-default-features", "--features", "embedded",
      "--target", target, "--crate-type", "rlib",
    }.ok, "no-std-target-check failed")
  end,
}

make.recipe{ name = "no-std-surface-check", desc = "check embedded public imports in a dedicated test",
             run = function() sh.cargo("check", "--no-default-features", "--features", "embedded", "--test", "no_std_surface") end }

make.recipe{
  name = "embedded-examples-check",
  desc = "check embedded-shaped examples",
  run = function()
    for _, example in ipairs{ "embedded_session_loop", "embedded_hal_adapter", "embedded_fixed_queue" } do
      sh.cargo("check", "--no-default-features", "--features", "embedded", "--example", example)
    end
  end,
}

make.recipe{
  name = "check-all",
  desc = ("check all targets with %s"):format(HOSTED_FEATURES),
  deps = { "no-std-surface-check" },
  run = function() sh.cargo("check", "--all-targets", "--features", HOSTED_FEATURES) end,
}

make.recipe{ name = "fmt", desc = "format the workspace",
             run = function() sh.cargo("fmt", "--all") end }

make.recipe{ name = "clippy", desc = "clippy with warnings denied",
             run = function() sh.cargo("clippy", "--all-targets", "--features", HOSTED_FEATURES, "--", "-D", "warnings") end }

make.recipe{
  name = "rustdoc",
  desc = "build docs with warnings denied",
  run = function()
    assert(oslo.run{ "env", "RUSTDOCFLAGS=-Dwarnings", "cargo", "doc", "--features", HOSTED_FEATURES, "--no-deps" }.ok,
           "rustdoc failed")
  end,
}

make.recipe{ name = "test-all", desc = ("test all targets with %s"):format(HOSTED_FEATURES),
             run = function() sh.cargo("test", "--all-targets", "--features", HOSTED_FEATURES) end }

make.recipe{ name = "clean", desc = "remove Cargo build artifacts",
             run = function() sh.cargo("clean") end }

make.recipe{ name = "bind", desc = "generate both C and Python bindings",
             deps = { "bind-c", "bind-py" } }

-- The C header is generated on nightly-only cbindgen internals, then rewritten into its
-- public form by the project's own generator script.
local function generated_header(dest)
  sh.cargo("build", "--lib")
  local tmp = oslo.run{ "mktemp", capture = true }
  assert(tmp.ok, "mktemp failed")
  local raw = (tmp.out or ""):gsub("%s+$", "")
  assert(oslo.run{ "env", "RUSTC_BOOTSTRAP=1", "cbindgen", "--config", "cbindgen.toml",
                    "--crate", NAME, "--output", raw }.ok, "cbindgen failed")
  sh.bash(GENERATE_C_ABI_PUBLIC_HEADER, raw, dest)
  oslo.run{ "rm", "-f", raw }
end

make.recipe{
  name = "bind-c",
  desc = "generate the C header",
  run = function() generated_header("include/" .. NAME .. ".h") end,
}

make.recipe{
  name = "bind-c-check",
  desc = "verify generated C header is up to date",
  run = function()
    local tmp = oslo.run{ "mktemp", "-d", capture = true }
    assert(tmp.ok, "mktemp -d failed")
    local dir = (tmp.out or ""):gsub("%s+$", "")
    sh.mkdir("-p", dir .. "/generated-header")
    generated_header(dir .. "/generated-header/" .. NAME .. ".h")
    local header_ok = oslo.run{ "diff", "-ru", "include/" .. NAME .. ".h", dir .. "/generated-header/" .. NAME .. ".h" }.ok
    local dir_ok = oslo.run{ "diff", "-ru", "include/" .. NAME, dir .. "/generated-header/" .. NAME }.ok
    if not (header_ok and dir_ok) then
      print("include/" .. NAME .. ".h is stale; run make bind-c")
      oslo.run{ "diff", "-ru", "include/" .. NAME .. ".h", dir .. "/generated-header/" .. NAME .. ".h" }
      oslo.run{ "diff", "-ru", "include/" .. NAME, dir .. "/generated-header/" .. NAME }
      oslo.run{ "rm", "-rf", dir }
      error("include/" .. NAME .. ".h is stale", 0)
    end
    oslo.run{ "rm", "-rf", dir }
  end,
}

make.recipe{ name = "bind-py", desc = "generate the Python bindings",
             run = function() sh.maturin("build", "--features", "pyo3/extension-module") end }

make.recipe{ name = "c-demo", desc = "build and run the basic C ABI demo",
             run = function() assert(oslo.run{ "make", "-C", "examples/c_abi", "run" }.ok, "c-demo failed") end }

make.recipe{ name = "c-full-demo", desc = "build and run the full C ABI demo",
             run = function() assert(oslo.run{ "make", "-C", "examples/c_abi", "run-full" }.ok, "c-full-demo failed") end }

make.recipe{ name = "python-demo", desc = "build and run the Python binding smokes plus wheel install",
             run = function() assert(oslo.run{ "make", "-C", "examples/python_binding", "test" }.ok, "python-demo failed") end }

make.recipe{
  name = "trace-replay-demo",
  desc = "replay compact/bracketed/malformed candump fixtures",
  run = function()
    for _, fixture in ipairs{
      "time_date_agisostack", "bracketed_time_date", "standard_id_rejection", "malformed_candump",
    } do
      sh.cargo("run", "--quiet", "--example", "candump_replay", "--",
               "tests/fixtures/traces/" .. fixture .. ".candump")
    end
  end,
}

make.recipe{
  name = "vt-evidence-smoke",
  desc = "run VT object-pool and command-trace evidence smokes",
  run = function()
    sh.cargo("run", "--quiet", "--example", "iop_inspect", "--",
      "--strict", "--physical-soft-keys", "10", "--navigation-soft-keys", "2",
      "--write-report-json", "/tmp/machbus-iop-inspect-report.json",
      "--write-rgb888", "/tmp/machbus-iop-inspect.rgb",
      "--write-rgb565-be", "/tmp/machbus-iop-inspect-be.rgb565",
      "--write-rgb565-le", "/tmp/machbus-iop-inspect-le.rgb565",
      "--expect-unsupported-records", "0", "--expect-placeholder-pixels", "0",
      "--expect-rgb888-fnv64", "0x527FEA44D2914422",
      "--expect-rgb565-be-fnv64", "0xC0F7DB231D7BC71F",
      "--expect-rgb565-le-fnv64", "0xA27374387E955487")
    sh.cargo("run", "--quiet", "--example", "vt_trace_inspect", "--",
      "--strict", "--physical-soft-keys", "10", "--navigation-soft-keys", "2",
      "--write-report-json", "/tmp/machbus-vt-trace-report.json",
      "--write-initial-rgb888", "/tmp/machbus-vt-trace-initial.rgb",
      "--write-final-rgb888", "/tmp/machbus-vt-trace-final.rgb",
      "--write-initial-rgb565-be", "/tmp/machbus-vt-trace-initial-be.rgb565",
      "--write-initial-rgb565-le", "/tmp/machbus-vt-trace-initial-le.rgb565",
      "--write-final-rgb565-be", "/tmp/machbus-vt-trace-final-be.rgb565",
      "--write-final-rgb565-le", "/tmp/machbus-vt-trace-final-le.rgb565",
      "--expect-accepted-effects", "3",
      "--expect-initial-placeholder-pixels", "0", "--expect-final-placeholder-pixels", "0",
      "--expect-rgb888-fnv64", "0xF7299A9637CEE405",
      "--expect-rgb565-be-fnv64", "0x54DC62D0E04D5405",
      "--expect-rgb565-le-fnv64", "0x54DC62D0E04D5405")
  end,
}

make.recipe{ name = "fuzz-smoke", desc = "run the arbitrary-input decoder fuzz-smoke test",
             run = function() sh.cargo("test", "--test", "fuzz_targets", "--", "--nocapture") end }

make.recipe{ name = "wirebit-examples-check", desc = "typecheck SocketCAN/vcan examples",
             run = function() sh.cargo("check", "--features", "wirebit", "--examples") end }

make.recipe{ name = "standard-suite-check", desc = "run the standard-derived ISO 11783/AEF/NMEA test suite",
             run = function() sh.cargo("test", "--test", "standard", "--", "--nocapture") end }

-- A generated/split/doc file named by position (part_1, chunk-2, …) instead of what it
-- contains, and any reference to one — checked with the same shell logic the Makefile used,
-- because it is a battery of find/grep patterns, not something restructuring clarifies.
make.recipe{
  name = "semantic-split-name-check",
  desc = "verify split source/header/doc names are content-based",
  run = function()
    local script = [[
set -e
bad="$(find . \
	\( -path './.git' \
	-o -path './target' \
	-o -path './book/book' \
	-o -path './examples/python_binding/.venv' \
	-o -path './examples/python_binding/.maturin' \) -prune -o \
	\( -type d -name '*_parts' \
	-o -type d -name '*_chunks' \
	-o -type d -name '*_sections' \
	-o -type d -name '*_slices' \
	-o -type d -name '*_pieces' \
	-o -type f -name 'part_[0-9]*.*' \
	-o -type f -name 'part-[0-9]*.*' \
	-o -type f -name 'chunk_[0-9]*.*' \
	-o -type f -name 'chunk-[0-9]*.*' \
	-o -type f -name 'section_[0-9]*.*' \
	-o -type f -name 'section-[0-9]*.*' \
	-o -type f -name 'slice_[0-9]*.*' \
	-o -type f -name 'slice-[0-9]*.*' \
	-o -type f -name 'piece_[0-9]*.*' \
	-o -type f -name 'piece-[0-9]*.*' \
	-o -type f -name 'split_[0-9]*.*' \
	-o -type f -name 'split-[0-9]*.*' \
	-o -type f -name 'parts-at-a-glance.*' \) -print)"
if [ -n "$bad" ]; then
	echo "non-semantic generated/split/doc file names found; name files after their contents instead:"
	printf '%s\n' "$bad"
	exit 1
fi
if git grep --untracked -n -E 'include!\("[^"]*_(parts|chunks|sections|slices|pieces)/[a-z]+_[0-9]+\.rs"\)|#include "[^"]*_(parts|chunks|sections|slices|pieces)/[a-z]+_[0-9]+\.h"' -- src tests include tools >/tmp/machbus-semantic-split-name-grep.txt; then
	echo "non-semantic split references found:"
	cat /tmp/machbus-semantic-split-name-grep.txt
	rm -f /tmp/machbus-semantic-split-name-grep.txt
	exit 1
fi
rm -f /tmp/machbus-semantic-split-name-grep.txt
if git grep --untracked -n -E '\]\([^)]*standards/(part-[0-9]+|parts-at-a-glance)[^)]*\)|standards/(part-[0-9]+|parts-at-a-glance)\.md' -- book/src >/tmp/machbus-semantic-doc-name-grep.txt; then
	echo "non-semantic standard document references found:"
	cat /tmp/machbus-semantic-doc-name-grep.txt
	rm -f /tmp/machbus-semantic-doc-name-grep.txt
	exit 1
fi
rm -f /tmp/machbus-semantic-doc-name-grep.txt
]]
    assert(oslo.run{ "sh", "-c", script }.ok, "semantic-split-name-check failed")
  end,
}

make.recipe{ name = "whitespace-check", desc = "verify git diff whitespace",
             run = function() sh.git("diff", "--check") end }

make.recipe{
  name = "book",
  desc = "build the documentation (mdBook in ./book)",
  run = function()
    need("mdbook", "mdbook is not installed; install it first")
    assert(oslo.fs.stat(TOP_DIR .. "/book/book.toml"), "book/book.toml does not exist")
    sh.mdbook("build", TOP_DIR .. "/book")
  end,
}

make.recipe{
  name = "verify",
  desc = "the whole local hardening gate",
  deps = {
    "check", "test", "check-all", "test-all", "clippy", "rustdoc", "bind-c-check",
    "c-demo", "c-full-demo", "python-demo", "trace-replay-demo", "fuzz-smoke",
    "wirebit-examples-check", "standard-suite-check", "semantic-split-name-check",
    "whitespace-check",
  },
}

make.recipe{
  name = "release",
  desc = "cut a version: --type patch | minor | major | M.m.p",
  params = { { "--type", desc = "patch | minor | major | M.m.p" } },
  run = function(a)
    need("git-rel", "git-rel is not installed; install it first")
    assert(type(a.type) == "string",
           "which release? make release --type patch|minor|major|M.m.p")
    sh.git("rel", a.type)
  end,
}
