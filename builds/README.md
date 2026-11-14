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

`builds/linux/` is populated by `scripts/build-linux.sh` on a Linux host, or by
the `Release` GitHub workflow on a `v*` tag. A Windows host cannot produce a
glibc-linked Linux GUI binary without a Linux sysroot, so use a Linux machine
or CI for the official Linux artifact.

The Linux package contains:

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
