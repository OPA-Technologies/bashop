# Installateur noarch de Bashop pour Windows / PowerShell.
#
# Détecte l'architecture, télécharge le binaire pré-compilé, vérifie son
# empreinte SHA256, l'installe sous %LOCALAPPDATA%\Bashop puis branche le
# bootstrap dans les profils PowerShell / cmd.
#
# Variables d'environnement :
#   BASHOP_VERSION   version à installer (défaut : latest)
#   BASHOP_INDEX     URL de l'index JSON
#   BASHOP_BIN       chemin d'un binaire local à installer au lieu de télécharger
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$Prefix = if ($env:PREFIX) { $env:PREFIX } else { Join-Path $env:LOCALAPPDATA 'Bashop' }
$Version = if ($env:BASHOP_VERSION) { $env:BASHOP_VERSION } else { 'latest' }
$IndexUrl = if ($env:BASHOP_INDEX) { $env:BASHOP_INDEX } else { 'https://raw.githubusercontent.com/OPA-Technologies/bashop/master/dist/index.json' }

function Write-Info([string]$Message)
  {
    Write-Host "bashop-install : $Message"
  }

function Get-Architecture
  {
    switch ($env:PROCESSOR_ARCHITECTURE)
      {
        'AMD64' { 'x86_64' }
        'ARM64' { 'aarch64' }
        'x86'   { 'x86_64' }
        default { throw "Architecture non prise en charge : $env:PROCESSOR_ARCHITECTURE" }
      }
  }

function Install-Bashop
  {
    $arch = Get-Architecture
    Write-Info "cible détectée : windows/$arch, Prefix=$Prefix"

    $binDir = Join-Path $Prefix 'bin'
    New-Item -ItemType Directory -Force -Path $binDir | Out-Null
    $target = Join-Path $binDir 'bashop.exe'

    if ($env:BASHOP_BIN)
      {
        if (-not (Test-Path $env:BASHOP_BIN)) { throw "binaire local introuvable : $($env:BASHOP_BIN)" }
        Copy-Item -Force $env:BASHOP_BIN $target
      }
    else
      {
        $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())
        New-Item -ItemType Directory -Force -Path $tmp | Out-Null
        try
          {
            Write-Info "récupération de l'index : $IndexUrl"
            $indexPath = Join-Path $tmp 'index.json'
            Invoke-WebRequest -UseBasicParsing -Uri $IndexUrl -OutFile $indexPath
            $index = Get-Content -Raw $indexPath | ConvertFrom-Json
            $asset = $index.assets | Where-Object { $_.os -eq 'windows' -and $_.arch -eq $arch } | Select-Object -First 1
            if (-not $asset) { throw "aucun binaire windows/$arch dans l'index." }

            $archive = Join-Path $tmp 'bashop.zip'
            Write-Info "téléchargement : $($asset.url)"
            Invoke-WebRequest -UseBasicParsing -Uri $asset.url -OutFile $archive

            if ($asset.sha256)
              {
                $sha = (Get-FileHash -Algorithm SHA256 -Path $archive).Hash.ToLower()
                if ($sha -ne $asset.sha256.ToLower()) { throw "SHA256 invalide (attendu $($asset.sha256), obtenu $sha)." }
              }

            Expand-Archive -Path $archive -DestinationPath $tmp -Force
            $found = Get-ChildItem -Path $tmp -Recurse -Filter 'bashop.exe' | Select-Object -First 1
            if (-not $found) { throw "binaire absent de l'archive." }
            Copy-Item -Force $found.FullName $target
          }
        finally
          {
            Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
          }
      }

    Write-Info "installé : $target"
    & $target install
    Write-Info "ajouter $binDir au PATH si nécessaire."
  }

Install-Bashop
