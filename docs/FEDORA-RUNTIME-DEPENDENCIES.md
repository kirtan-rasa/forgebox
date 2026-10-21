# ForgeBox on Fedora

The RPM declares the common conversion engines as package dependencies. Installing the RPM with DNF therefore installs the application and these dependencies together; the user does not need to run the source-build dependency script just to install the application.

```bash
sudo dnf install ./forgebox-*.rpm
```

English OCR data is installed as a requirement because it is the default OCR language. Russian OCR data and FFmpeg are weak/recommended dependencies, not hard requirements. FFmpeg may require RPM Fusion to be enabled. Background removal (`rembg`) remains a separate optional Python environment.
