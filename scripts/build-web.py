"""Build the static GitHub Pages artifact using the locked Rust dependencies."""
import pathlib
import os
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / "target" / "web"
VERSION = "0.2.126"


def run(*args):
    env = os.environ.copy()
    # Rust panic locations must not embed the builder's home directory in the
    # published WASM. Encoded flags also handle workspace paths with spaces.
    flags = env["CARGO_ENCODED_RUSTFLAGS"].split("\x1f") if env.get("CARGO_ENCODED_RUSTFLAGS") else env.get("RUSTFLAGS", "").split()
    flags.extend([f"--remap-path-prefix={pathlib.Path.home()}=/build", f"--remap-path-prefix={ROOT}=."])
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
    subprocess.run(args, cwd=ROOT, check=True, env=env)


def main():
    tool = shutil.which("wasm-bindgen")
    if not tool:
        raise SystemExit(f"Install the packager first: cargo install wasm-bindgen-cli --version {VERSION} --locked")
    version = subprocess.check_output([tool, "--version"], text=True).strip()
    if version != f"wasm-bindgen {VERSION}":
        raise SystemExit(f"Expected wasm-bindgen {VERSION}; found {version}")
    run("cargo", "build", "--locked", "--release", "--target", "wasm32-unknown-unknown")
    OUT.mkdir(parents=True, exist_ok=True)
    run(tool, "--target", "web", "--out-name", "voxelpopuli", "--out-dir", str(OUT / "pkg"),
        str(ROOT / "target" / "wasm32-unknown-unknown" / "release" / "VoxelPopuli.wasm"))
    for name in ["index.html", "style.css", "boot.js"]:
        shutil.copyfile(ROOT / "web" / name, OUT / name)
    # wasm-bindgen copies directly referenced modules, but not their imports.
    for runtime in (OUT / "pkg" / "snippets").rglob("runtime.js"):
        shutil.copyfile(ROOT / "web" / "storage.js", runtime.with_name("storage.js"))
    wasm = (OUT / "pkg" / "voxelpopuli_bg.wasm").read_bytes()
    home = pathlib.Path.home()
    if any(prefix.encode() in wasm for prefix in (str(home), home.as_posix())):
        raise SystemExit("The WASM artifact still contains the builder's home directory")
    (OUT / ".nojekyll").touch()
    print("Built target/web. Preview: python -m http.server 8080 --directory target/web")


if __name__ == "__main__":
    main()
