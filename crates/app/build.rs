fn main() {
    // Translations in lang/<language>/LC_MESSAGES/acore-launcher.po are built in;
    // Slint picks the one matching the system language.
    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent-dark".into())
        .with_bundled_translations("lang");
    slint_build::compile_with_config("ui/main.slint", config).expect("Slint build failed");

    // Windows: icon, file details and a manifest. asInvoker keeps Windows from
    // guessing the launcher is an installer that needs admin rights.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("ui/assets/icon.ico")
            .set("FileDescription", "Game launcher")
            .set("ProductName", "Game launcher")
            .set_manifest(
                r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security><requestedPrivileges>
      <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
    </requestedPrivileges></security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>"#,
            );
        res.compile().expect("Windows resources");
    }
}
