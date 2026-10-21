#[cfg(windows)]
fn main() {
    println!("cargo:rerun-if-changed=assets/forgebox.ico");
    let mut resources = winres::WindowsResource::new();
    resources.set_icon("assets/forgebox.ico");
    resources
        .compile()
        .expect("failed to embed ForgeBox icon into the Windows executable");
}

#[cfg(not(windows))]
fn main() {
    println!("cargo:rerun-if-changed=assets/forgebox.ico");
}
