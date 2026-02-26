Name:           sedx
Version:        0.2.6_alpha
Release:        1%{?dist}
Summary:        A safe, modern replacement for GNU sed with automatic backups and rollback

License:        MIT
URL:            https://github.com/InkyQuill/sedx
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz

# Build dependencies
BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc

# Runtime dependencies
Requires:       glibc

%description
SedX is a safe, modern replacement for GNU sed written in Rust. It provides
safe file editing with automatic backups, dry-run mode, and easy rollback.

Key features:
- Automatic backups before every modification
- Dry-run mode to preview changes
- One-command rollback of any operation
- Colored diff output with context
- PCRE (modern regex) by default
- ~90% GNU sed compatibility
- Streaming mode for large files (constant memory usage)

%prep
%autosetup -n %{name}-%{version}

%build
cargo build --release

%install
# Install binary
install -Dm755 target/release/sedx %{buildroot}%{_bindir}/sedx

# Install man page
install -Dm644 man/sedx.1 %{buildroot}%{_mandir}/man1/sedx.1

# Install bash completion
mkdir -p %{buildroot}%{_datadir}/bash-completion/completions
./target/release/sedx --completions bash > %{buildroot}%{_datadir}/bash-completion/completions/sedx

# Install zsh completion
mkdir -p %{buildroot}%{_datadir}/zsh/site-functions
./target/release/sedx --completions zsh > %{buildroot}%{_datadir}/zsh/site-functions/_sedx

# Install fish completion
mkdir -p %{buildroot}%{_datadir}/fish/vendor_completions.d
./target/release/sedx --completions fish > %{buildroot}%{_datadir}/fish/vendor_completions.d/sedx.fish

# Install license
install -Dm644 LICENSE %{buildroot}%{_licensedir}/LICENSE

# Install documentation
install -Dm644 README.md %{buildroot}%{_docdir}/%{name}/README.md

%check
cargo test --release

%files
%license LICENSE
%doc README.md
%{_bindir}/sedx
%{_mandir}/man1/sedx.1*
%{_datadir}/bash-completion/completions/sedx
%{_datadir}/zsh/site-functions/_sedx
%{_datadir}/fish/vendor_completions.d/sedx.fish

%changelog
* %(date "+%%a %%b %%d %%Y") InkyQuill <inkyquill@users.noreply.github.com> - 0.2.6_alpha-1
- Initial package release
