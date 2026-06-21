$jdkPath = "C:\Program Files\Microsoft\jdk-25.0.3.9-hotspot"
$java = Join-Path $jdkPath "bin\java.exe"
if (-not (Test-Path $java)) {
    $java = "java"
}

$jars = Get-ChildItem -Path "lib" -Filter "*.jar" | ForEach-Object { $_.FullName }
$classpath = ($jars -join ";") + ";target\classes"

Write-Host "Running VoxelPopuli..."
& $java --enable-native-access=ALL-UNNAMED --sun-misc-unsafe-memory-access=allow -cp $classpath com.voxelpopuli.client.Main
