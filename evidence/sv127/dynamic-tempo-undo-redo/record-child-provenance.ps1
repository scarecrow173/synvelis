$ErrorActionPreference = 'Stop'
$root = (& git rev-parse --show-toplevel).Trim()
if ($LASTEXITCODE -ne 0 -or @(git status --porcelain --untracked-files=no).Count) {
    throw 'Run from a clean tracked checkout before editing the evidence.'
}
Set-Location $root
$env:TIM03_VERIFIED_SHA = (& git rev-parse HEAD).Trim()
$override = 'SYNVELIS_TIM03_L1_PLUGIN_DIR'
$overrideWasPresent = Test-Path "Env:$override"
Remove-Item "Env:$override" -ErrorAction SilentlyContinue
if (Test-Path "Env:$override") { throw 'Runner DLL directory override remains present' }

$filter = 'production_tempo_undo_redo'
$task = 'test-tim03-dynamic-musical-transport'
$formats = [ordered]@{
    clap = '13319ad9c0de9531'
    vst3 = '1ac2c390d2654dc3'
}
$artifacts = [ordered]@{}
foreach ($format in $formats.Keys) {
    $stem = "ruviform_plugin_test_$format"
    $fixture = (Resolve-Path "target/debug/deps/$stem-$($formats[$format]).dll").Path
    $runnerName = "synvelis-plugin-test-$format-l1-runner"
    $runner = (Resolve-Path "target/debug/$runnerName.exe").Path
    $sibling = Join-Path (Split-Path $runner) "$stem.dll"
    $envName = 'SYNVELIS_TIM03_REAL_ABI_' + $format.ToUpperInvariant()
    Set-Item "Env:$envName" $fixture
    $envName = 'SYNVELIS_TIM03_L1_' + $format.ToUpperInvariant() + '_RUNNER'
    Set-Item "Env:$envName" $runner
    $artifacts[$format] = @{
        runner_name = $runnerName
        runner = $runner
        fixture = $fixture
        sibling = $sibling
        fixture_sha256_before = (Get-FileHash -LiteralPath $fixture).Hash.ToLowerInvariant()
        sibling_sha256_before = (Get-FileHash -LiteralPath $sibling).Hash.ToLowerInvariant()
    }
}

$os = Get-CimInstance Win32_OperatingSystem
$version = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
$meta = [ordered]@{
    command = "mise run $task -- $filter"
    tested_sha = $env:TIM03_VERIFIED_SHA
    start_utc = [DateTime]::UtcNow.ToString('o')
    os = @{
        caption = $os.Caption
        version = $os.Version
        build = $os.BuildNumber
        update_build_revision = $version.UBR
        display_version = $version.DisplayVersion
        architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    }
    plugin_dir_override_present_before_clear = $overrideWasPresent
    plugin_dir_override_present_at_launch = Test-Path "Env:$override"
}

$temporary = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("tim03-" + [guid]::NewGuid()))
$stdout = Join-Path $temporary.FullName 'stdout'
$stderr = Join-Path $temporary.FullName 'stderr'
$mise = (Get-Command mise -CommandType Application).Source
$lane = Start-Process -FilePath $mise -ArgumentList @('run', $task, '--', $filter) `
    -NoNewWindow -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
$observed = [ordered]@{}
while (-not $lane.HasExited) {
    foreach ($format in $formats.Keys) {
        if ($observed.Contains($format)) { continue }
        $artifact = $artifacts[$format]
        foreach ($runner in @(Get-Process -Name $artifact.runner_name -ErrorAction SilentlyContinue)) {
            try {
                $exe = $runner.Path
                $dll = $runner.Modules | Where-Object { $_.ModuleName -eq "ruviform_plugin_test_$format.dll" } |
                    Select-Object -First 1 -ExpandProperty FileName
                if (-not $dll -or -not $exe) { continue }
                $child = Get-CimInstance Win32_Process -Filter "ProcessId = $($runner.Id)"
                if (-not $child) { continue }
                $parent = Get-CimInstance Win32_Process -Filter "ProcessId = $($child.ParentProcessId)"
                if (-not $parent -or $parent.Name -notlike 'tim03_l1_production_musical_transport-*.exe' `
                    -or $parent.CommandLine -notlike "*$filter*") { continue }
                $observed[$format] = @{
                    observed_utc = [DateTime]::UtcNow.ToString('o')
                    child_pid = $runner.Id
                    child_exe = $exe
                    parent_pid = $parent.ProcessId
                    parent_command_line = $parent.CommandLine
                    loaded_module = $dll
                    loaded_sha256 = (Get-FileHash -LiteralPath $dll).Hash.ToLowerInvariant()
                    child_exe_sha256 = (Get-FileHash -LiteralPath $exe).Hash.ToLowerInvariant()
                    is_executable_sibling = $dll -eq (Join-Path (Split-Path $exe) "ruviform_plugin_test_$format.dll")
                }
            } catch {
                if (-not $runner.HasExited) { throw }
            }
        }
    }
    Start-Sleep -Milliseconds 40
}
$lane.WaitForExit()
$meta['exit_code'] = $lane.ExitCode
$meta['end_utc'] = [DateTime]::UtcNow.ToString('o')
foreach ($format in $formats.Keys) {
    $artifact = $artifacts[$format]
    $artifact['fixture_sha256_after'] = (Get-FileHash -LiteralPath $artifact.fixture).Hash.ToLowerInvariant()
    $artifact['sibling_sha256_after'] = (Get-FileHash -LiteralPath $artifact.sibling).Hash.ToLowerInvariant()
}
$meta['artifacts'] = $artifacts
$meta['observed_child_modules'] = $observed
$log = Join-Path $PSScriptRoot 'dynamic-child-provenance.log'
$utf8 = New-Object System.Text.UTF8Encoding($false)
$header = 'tim03-child-provenance ' + ($meta | ConvertTo-Json -Depth 8 -Compress)
$text = $header + "`n" + [IO.File]::ReadAllText($stderr) + [IO.File]::ReadAllText($stdout)
[IO.File]::WriteAllText($log, $text.Replace("`r`n", "`n").TrimEnd() + "`n", $utf8)
Write-Output "lane exit=$($meta.exit_code); observed child modules=$($observed.Keys -join ','); log=$log"
if ($meta.exit_code -ne 0) { exit $meta.exit_code }
if ($observed.Count -ne $formats.Count) { exit 1 }
