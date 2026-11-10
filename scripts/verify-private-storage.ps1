param([Parameter(Mandatory = $true)][string]$Path)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Security
$envelope = [IO.File]::ReadAllBytes($Path)
if ($envelope.Length -le 65) { throw 'Truncated envelope.' }
$magic = [Text.Encoding]::ASCII.GetString($envelope,0,16)
if ($magic -ne "FOLIO-SEALED-V1`0" -or $envelope[16] -ne 2) { throw 'Unexpected format or purpose.' }
$plainLength = [BitConverter]::ToUInt64($envelope,17)
$cipherLength = [BitConverter]::ToUInt64($envelope,25)
if ($cipherLength -ne $envelope.Length - 65) { throw 'Incorrect ciphertext length.' }
$ciphertext = New-Object byte[] ($envelope.Length - 65)
[Array]::Copy($envelope,65,$ciphertext,0,$ciphertext.Length)
$hash = [Security.Cryptography.SHA256]::Create()
try { $digest = $hash.ComputeHash($ciphertext) } finally { $hash.Dispose() }
for ($i = 0; $i -lt 32; $i++) { if ($digest[$i] -ne $envelope[33+$i]) { throw 'Ciphertext checksum mismatch.' } }
$entropy = [Text.Encoding]::UTF8.GetBytes('Folio recovery v1/source')
$inner = [Security.Cryptography.ProtectedData]::Unprotect($ciphertext,$entropy,[Security.Cryptography.DataProtectionScope]::CurrentUser)
if ($inner.Length -ne 57 + $plainLength) { throw 'Incorrect protected payload length.' }
for ($i = 0; $i -lt 25; $i++) { if ($inner[$i] -ne $envelope[$i]) { throw 'Protected header mismatch.' } }
$plaintext = New-Object byte[] ([int]$plainLength)
[Array]::Copy($inner,57,$plaintext,0,$plaintext.Length)
$hash = [Security.Cryptography.SHA256]::Create()
try { $digest = $hash.ComputeHash($plaintext) } finally { $hash.Dispose() }
for ($i = 0; $i -lt 32; $i++) { if ($digest[$i] -ne $inner[25+$i]) { throw 'Digest mismatch.' } }
if ([Text.Encoding]::UTF8.GetString($plaintext) -ne 'Folio synthetic DPAPI interoperability probe') { throw 'Probe text mismatch.' }
[ordered]@{ independentDpapiUnseal=$true; protectedHeaderMatches=$true; checksumMatches=$true; syntheticPayloadMatches=$true } | ConvertTo-Json
[Array]::Clear($inner,0,$inner.Length)
[Array]::Clear($plaintext,0,$plaintext.Length)
