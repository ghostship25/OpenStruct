# Build artifacts

Binaries here are produced from the source in this repository. They are
convenience downloads, not a signed release channel.

## Windows

`builds/windows/` contains:

- `OpenStruct.exe` — desktop application.
- `openstruct-cli.exe` — headless analysis CLI.
- `openstruct.dll` — the engine C ABI.
- `SHA256SUMS.txt` — checksums for the three files.

Rebuild with:

```powershell
.\scripts\build-windows.ps1
```

## Linux

`builds/linux/` contains the `v0.1.0` package built by the `Release` GitHub
workflow on `ubuntu-latest`; rebuild it on a Linux host with
`scripts/build-linux.sh`. A Windows host cannot link a glibc/X11 desktop binary
without a Linux sysroot. The Linux package contains:

- `OpenStruct` — desktop application.
- `openstruct-cli` — headless analysis CLI.
- `libopenstruct.so` — the engine C ABI.
- `SHA256SUMS.txt` — checksums.

## Verify a download

```bash
sha256sum -c SHA256SUMS.txt
```

```powershell
Get-FileHash .\OpenStruct.exe -Algorithm SHA256
```
