$jdkPath = "C:\Program Files\Microsoft\jdk-25.0.3.9-hotspot"
$javac = Join-Path $jdkPath "bin\javac.exe"

if (-not (Test-Path $javac)) {
    $javac = "javac"
}

# Create build directory
New-Item -ItemType Directory -Force target\classes

# Find all java files
$javaFiles = Get-ChildItem -Recurse -Filter "*.java" -Path "src/main/java" | ForEach-Object { $_.FullName }

if ($javaFiles.Count -eq 0) {
    Write-Host "No Java files found to compile."
    exit
}

# Collect all jar files in lib
$jars = Get-ChildItem -Path "lib" -Filter "*.jar" | ForEach-Object { $_.FullName }
$classpath = ($jars -join ";") + ";target\classes"

Write-Host "Compiling $($javaFiles.Count) Java source files..."
& $javac -d target\classes -cp $classpath $javaFiles

if ($LASTEXITCODE -eq 0) {
    Write-Host "Compilation successful!"
} else {
    Write-Error "Compilation failed."
    exit $LASTEXITCODE
}
