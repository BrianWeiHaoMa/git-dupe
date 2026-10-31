# Installs the latest release of git-dupe on this Windows machine: the executable into
# ~\.local\bin, the one copy the release's INSTALL-windows.txt makes. Paste it into
# PowerShell as
#
#   irm https://raw.githubusercontent.com/BrianWeiHaoMa/git-dupe/main/install.ps1 | iex
#
# It reads the latest release's tag from the releases pages, downloads the zip and its
# .sha256, checks the zip's SHA-256 digest, unpacks it, and runs the executable once; only
# then does it place the file, by a rename, so that the path holds the old file, for an
# instant nothing, or the new one, and never part of one. Every failure before that
# installs nothing. It prints the line that puts ~\.local\bin on PATH and runs it for
# nobody: it changes no setting of the user's, and a success leaves the calling session as
# it was, where a refusal exits, as a script's failure does.
# It runs under Windows PowerShell 5.1, which Windows 11 ships, and needs nothing else. It
# never reads standard input, and writes nothing else outside a temporary directory it
# removes.
#
# GIT_DUPE_RELEASES_URL, when set, replaces the base of the releases pages,
# https://github.com/BrianWeiHaoMa/git-dupe/releases, for the repository's checks, which
# point it at a stand-in on the loopback interface, and for a mirror serving the same
# paths: <base>/latest redirecting to <base>/tag/<tag>, and each file of a release under
# <base>/download/<tag>/. Only this script and install.sh read it.
#
# Every command stands in a function and the last line calls the main one, because iex runs
# the text it is given: a download cut short inside a function is a parse error, and one cut
# between functions defines them and runs nothing. No statement stands beside them, not
# even a preference, because a cut prefix would run it.

function Write-GitDupeLine([string]$Line) {
    # Past the formatter, which would wrap the long PATH line it prints for pasting.
    [Console]::Out.WriteLine($Line)
}

# Where a Windows build is no use: another system, or another architecture.
function Assert-GitDupeMachine([string]$FromSource) {
    if ([Environment]::OSVersion.Platform -ne 'Win32NT') {
        throw 'install.ps1 is for Windows, and install.sh for this system; nothing was installed.'
    }
    $machine = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    if ($machine -ne 'X64') {
        throw "no release of git-dupe is built for Windows on $machine; install it from source instead, as $FromSource says. Nothing was installed."
    }
}

# The latest release's tag, v<version>, from where <base>/latest redirects, which is read
# and not followed: the release's page says nothing a script should parse.
function Get-GitDupeTag([string]$Releases) {
    $latest = "$Releases/latest"
    try {
        $request = [Net.HttpWebRequest]::Create($latest)
        $request.AllowAutoRedirect = $false
        $request.Method = 'HEAD'
        $response = $request.GetResponse()
        $location = $response.Headers['Location']
        $response.Close()
    } catch {
        throw "cannot find the latest release at ${latest}; nothing was installed."
    }
    if (-not $location -or -not $location.StartsWith("$Releases/tag/", [StringComparison]::Ordinal)) {
        throw "$latest led to '$location', which is no release's page; nothing was installed."
    }
    $tag = $location.Substring("$Releases/tag/".Length)
    if ($tag -cnotmatch '\Av[0-9][0-9A-Za-z.-]*\z') {
        throw "the latest release's tag, $tag, is not v<version>; nothing was installed."
    }
    return $tag
}

function Save-GitDupeFile([string]$Url, [string]$To) {
    try {
        Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $To
    } catch {
        throw "cannot download ${Url}; nothing was installed."
    }
}

# The .sha256 is read here: it must be the one line a release writes, the digest and the
# zip's bare name, or the line Git Bash's sha256sum writes, and that digest the zip's own.
# Get-FileHash prints upper case, and a release lower.
function Assert-GitDupeDigest([string]$Zip, [string]$Sidecar) {
    $name = Split-Path -Leaf $Zip
    $published = [IO.File]::ReadAllText($Sidecar)
    $line = '\A([0-9A-Fa-f]{64})(?:  | \*)' + [regex]::Escape($name) + '\r?\n?\z'
    if ($published -cnotmatch $line) {
        throw "$name.sha256 is not the SHA-256 digest of $name followed by its name; nothing was installed."
    }
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $Zip).Hash -ine $Matches[1]) {
        throw "$name does not have the SHA-256 digest its $name.sha256 names; nothing was installed."
    }
}

# Unpacks the zip and runs its executable before anything is installed, so that one that
# cannot run here fails here and not at the first `git dupe`. Returns its path.
function Expand-GitDupe([string]$Zip, [string]$Work, [string]$Version, [string]$FromSource) {
    $name = Split-Path -Leaf $Zip
    try {
        Expand-Archive -LiteralPath $Zip -DestinationPath $Work -ErrorAction Stop
    } catch {
        throw "cannot unpack $name; nothing was installed."
    }
    $executable = Join-Path $Work "git-dupe-$Version\git-dupe.exe"
    if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
        throw "$name holds no git-dupe-$Version\git-dupe.exe; nothing was installed."
    }
    try {
        $null = & $executable --version
        $ran = $LASTEXITCODE -eq 0
    } catch {
        $ran = $false
    }
    if (-not $ran) {
        throw "the git-dupe of $name does not run on this machine; install it from source instead, as $FromSource says. Nothing was installed."
    }
    return $executable
}

# Places $From at $Target through a fresh file in its directory, renamed onto it. A running
# executable cannot be replaced and can be renamed away, so that what stands there goes
# aside first; its removal is refused while it runs, and then it stays for the next run,
# which removes it, unless a run killed between the renames left it the only copy, which
# stays until the new file stands. Returns whether it stayed.
function Install-GitDupeFile([string]$From, [string]$Target) {
    $aside = "$Target.old"
    foreach ($path in $Target, $aside) {
        if (Test-Path -LiteralPath $path -PathType Container) {
            throw "a directory stands at $path, where a file belongs; nothing was installed."
        }
    }
    try {
        if ((Test-Path -LiteralPath $aside) -and (Test-Path -LiteralPath $Target)) {
            Remove-Item -LiteralPath $aside -Force
        }
    } catch {
        throw "cannot remove $aside, an earlier upgrade's executable that may still run: run this again once it has ended; nothing was installed."
    }
    $directory = Split-Path -Parent $Target
    $fresh = Join-Path $directory ('.git-dupe.exe.' + [IO.Path]::GetRandomFileName())
    try {
        $null = New-Item -ItemType Directory -Force -Path $directory
        Copy-Item -LiteralPath $From -Destination $fresh
        $earlier = Test-Path -LiteralPath $Target
        if ($earlier) {
            Move-Item -LiteralPath $Target -Destination $aside
        }
        try {
            Move-Item -LiteralPath $fresh -Destination $Target
        } catch {
            if ($earlier) {
                Move-Item -LiteralPath $aside -Destination $Target
            }
            throw
        }
    } catch {
        throw "cannot place git-dupe.exe in ${directory}: $($_.Exception.Message); nothing was installed."
    } finally {
        Remove-Item -LiteralPath $fresh -Force -ErrorAction SilentlyContinue
    }
    if (-not (Test-Path -LiteralPath $aside)) {
        return $false
    }
    try {
        Remove-Item -LiteralPath $aside -Force
        return $false
    } catch {
        return $true
    }
}

# A warning only: the executable installs the same whatever Git is on PATH.
function Test-GitDupeGit {
    $wanted = 'git-dupe needs Git 2.43.0 or newer, below 3.0.0'
    if (-not (Get-Command git -CommandType Application -ErrorAction SilentlyContinue)) {
        [Console]::Error.WriteLine("install.ps1: warning: no git is on PATH; $wanted.")
        return
    }
    try {
        $said = [string](& git --version)
    } catch {
        $said = ''
    }
    if ($said -cnotmatch '\Agit version ([0-9]+)\.([0-9]+)') {
        [Console]::Error.WriteLine("install.ps1: warning: cannot read the version of the git on PATH; $wanted.")
    } elseif ([int]$Matches[1] -ne 2 -or [int]$Matches[2] -lt 43) {
        [Console]::Error.WriteLine("install.ps1: warning: the git on PATH is $said; $wanted.")
    }
}

function Write-GitDupeReport([string]$Target, [bool]$Aside) {
    Write-GitDupeLine "Installed ${Target}:"
    Write-GitDupeLine ([string](& $Target --version))
    if ($Aside) {
        Write-GitDupeLine "The earlier executable still runs, as $Target.old, which the next run removes."
    }
    $bin = Split-Path -Parent $Target
    $onPath = ($env:Path -split ';' | ForEach-Object { $_.TrimEnd('\') }) -contains $bin
    if (-not $onPath) {
        Write-GitDupeLine ''
        Write-GitDupeLine "Add $bin to PATH, where Git looks for git-dupe: it is not on PATH now. This line adds it for this session:"
        Write-GitDupeLine '    $env:Path = "$HOME\.local\bin;$env:Path"'
        Write-GitDupeLine 'and this one, run once, for every new shell:'
        Write-GitDupeLine "    [Environment]::SetEnvironmentVariable('Path', `"`$HOME\.local\bin;`" + [Environment]::GetEnvironmentVariable('Path', 'User'), 'User')"
    }
    Test-GitDupeGit
}

function Install-GitDupeRelease([string]$Releases, [string]$Work) {
    $fromSource = 'https://github.com/BrianWeiHaoMa/git-dupe/blob/main/docs/install.md#from-source'
    Assert-GitDupeMachine $fromSource
    $tag = Get-GitDupeTag $Releases
    $version = $tag.Substring(1)
    $name = "git-dupe-$version-windows-x86_64.zip"
    $zip = Join-Path $Work $name
    Write-GitDupeLine "Downloading $name from $Releases/download/$tag/"
    Save-GitDupeFile "$Releases/download/$tag/$name" $zip
    Save-GitDupeFile "$Releases/download/$tag/$name.sha256" "$zip.sha256"
    Assert-GitDupeDigest $zip "$zip.sha256"
    $executable = Expand-GitDupe $zip $Work $version $fromSource
    $target = Join-Path $HOME '.local\bin\git-dupe.exe'
    $aside = Install-GitDupeFile $executable $target
    Write-GitDupeReport $target $aside
}

# Every preference stands here, inside the function, and so ends with it; a success returns
# rather than exits, so that what follows the pasted line in its session runs.
function Install-GitDupe {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    $exitCode = $global:LASTEXITCODE
    $releases = 'https://github.com/BrianWeiHaoMa/git-dupe/releases'
    if ($env:GIT_DUPE_RELEASES_URL) {
        $releases = $env:GIT_DUPE_RELEASES_URL -replace '/$', ''
    }
    $work = Join-Path $env:TEMP ('git-dupe-install.' + [IO.Path]::GetRandomFileName())
    $refusal = $null
    try {
        $null = New-Item -ItemType Directory -Path $work
        Install-GitDupeRelease $releases $work
    } catch {
        $refusal = $_.Exception.Message
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
        $global:LASTEXITCODE = $exitCode
    }
    if ($refusal) {
        [Console]::Error.WriteLine("install.ps1: $refusal")
        exit 1
    }
}

Install-GitDupe
