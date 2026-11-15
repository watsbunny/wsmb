# Vendored OpenFX headers

`include/` contains the public headers of the OpenFX image processing API,
vendored unmodified from
<https://github.com/AcademySoftwareFoundation/openfx> (`main`, OFX 1.5).

They are **not compiled** by this project — they are the reference for the hand
written FFI declarations in `crates/wsmb-ofx/src/sys.rs`, so that field order and
signatures can be audited against the official source.

Upstream licence: BSD-3-Clause, see `LICENSE` in this directory.

To refresh:

```powershell
curl.exe -sSL -o include/ofxCore.h https://raw.githubusercontent.com/AcademySoftwareFoundation/openfx/main/include/ofxCore.h
# ... same for the other headers listed in the project README
```
