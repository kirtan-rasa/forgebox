Name:           forgebox
Version:        %{forgebox_version}
Release:        %{forgebox_release}%{?dist}
Summary:        Local file conversion and utility studio
License:        MIT
Source0:        forgebox
Source1:        forgebox.png
Source2:        forgebox.desktop
Source3:        LICENSE
Source4:        README.md
Source5:        install-rembg-fedora.sh
BuildArch:      x86_64

# Required command-line engines used by the app's conversion tools.
Requires:       ImageMagick
Requires:       poppler-utils
Requires:       ghostscript
Requires:       qpdf
Requires:       pdftk-java
Requires:       libreoffice
Requires:       tesseract
Requires:       tesseract-langpack-eng
Requires:       perl-Image-ExifTool
Requires:       xdg-utils

# Optional backends are weak dependencies so they do not make installation
# fail on systems where their repositories are not enabled.
Recommends:     ffmpeg
Recommends:     tesseract-langpack-rus

%description
ForgeBox is a desktop utility for local image, PDF, document, spreadsheet,
audio, and video conversion. It provides one graphical interface and calls
system conversion utilities only for features that use them.

%prep

%build

%install
install -D -m 0755 %{SOURCE0} %{buildroot}%{_bindir}/forgebox
install -D -m 0644 %{SOURCE1} %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/forgebox.png
install -D -m 0644 %{SOURCE2} %{buildroot}%{_datadir}/applications/forgebox.desktop
install -D -m 0644 %{SOURCE3} %{buildroot}%{_datadir}/licenses/%{name}/LICENSE
install -D -m 0644 %{SOURCE4} %{buildroot}%{_docdir}/%{name}/README.md
install -D -m 0644 %{SOURCE5} %{buildroot}%{_docdir}/%{name}/install-rembg-fedora.sh

%files
%{_bindir}/forgebox
%{_datadir}/icons/hicolor/256x256/apps/forgebox.png
%{_datadir}/applications/forgebox.desktop
%license %{_datadir}/licenses/%{name}/LICENSE
%doc %{_docdir}/%{name}/README.md
%doc %{_docdir}/%{name}/install-rembg-fedora.sh

