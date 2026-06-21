$urls = @(
    "https://repo1.maven.org/maven2/org/lwjgl/lwjgl/3.3.4/lwjgl-3.3.4.jar",
    "https://repo1.maven.org/maven2/org/lwjgl/lwjgl/3.3.4/lwjgl-3.3.4-natives-windows.jar",
    "https://repo1.maven.org/maven2/org/lwjgl/lwjgl-glfw/3.3.4/lwjgl-glfw-3.3.4.jar",
    "https://repo1.maven.org/maven2/org/lwjgl/lwjgl-glfw/3.3.4/lwjgl-glfw-3.3.4-natives-windows.jar",
    "https://repo1.maven.org/maven2/org/lwjgl/lwjgl-opengl/3.3.4/lwjgl-opengl-3.3.4.jar",
    "https://repo1.maven.org/maven2/org/lwjgl/lwjgl-opengl/3.3.4/lwjgl-opengl-3.3.4-natives-windows.jar",
    "https://repo1.maven.org/maven2/org/joml/joml/1.10.8/joml-1.10.8.jar"
)

New-Item -ItemType Directory -Force lib
foreach ($url in $urls) {
    $filename = Split-Path $url -Leaf
    $dest = Join-Path "lib" $filename
    if (-not (Test-Path $dest)) {
        Write-Host "Downloading $filename..."
        Invoke-WebRequest -Uri $url -OutFile $dest
    }
}
Write-Host "All libraries downloaded."
