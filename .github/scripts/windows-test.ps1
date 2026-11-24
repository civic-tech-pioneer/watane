# Tauri's build script embeds the Common Controls v6 manifest in the app binary.
# The app library's test executable also imports those controls.
$ErrorActionPreference = 'Stop'
$executable, $testArgs = $args
if ([IO.Path]::GetFileName($executable) -like 'watane_app-*.exe') {
    @'
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
</assembly>
'@ | Set-Content -LiteralPath "$executable.manifest" -Encoding utf8
}
& $executable @testArgs
exit $LASTEXITCODE
