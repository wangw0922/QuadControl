if (-not (Get-Command dotnet -ErrorAction SilentlyContinue)) {
  Write-Output "SKIP: dotnet is not installed; Windows host is not built."
  exit 0
}
Write-Output "SKIP: no Windows solution exists in M0; nothing to build."
