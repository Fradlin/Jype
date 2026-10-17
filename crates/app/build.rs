//! Embeds the icons, version information (publisher: Frad) and the manifest into Jype.exe.

fn main() {
    println!("cargo:rerun-if-changed=assets/jype.ico");
    println!("cargo:rerun-if-changed=assets/jype_off.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/jype.ico"); // id 1: exe icon + tray "on"
    res.set_icon_with_id("assets/jype_off.ico", "2"); // tray "off"
    res.set("CompanyName", "Frad");
    res.set("ProductName", "Jype");
    res.set("FileDescription", "Jype - Bangla typing for Windows");
    res.set("LegalCopyright", "Copyright (c) Frad");
    res.set("OriginalFilename", "Jype.exe");
    res.set("InternalName", "Jype");
    res.set_manifest(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="Frad.Jype" version="0.3.0.0"/>
  <description>Jype - Bangla typing</description>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
        processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security><requestedPrivileges>
      <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
    </requestedPrivileges></security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
"#,
    );
    if let Err(e) = res.compile() {
        // Never fail the whole build because of resources; the app still works.
        println!("cargo:warning=could not embed resources: {e}");
    }
}
