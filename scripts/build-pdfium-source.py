"""Build the locked Windows x64 PDFium dependency in an isolated short-path cache.

Invoked by build-pdfium.ps1. Never changes installed SDKs or existing resources.
The output is a separate candidate; native/desktop acceptance remains required.
"""
import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import urllib.request

WORKSPACE = Path(__file__).resolve().parent.parent
LOCK = json.loads((WORKSPACE / 'scripts/pdfium-source-lock.json').read_text())


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def run(*args, cwd=None):
    print('+', *map(str, args), flush=True)
    subprocess.run(list(map(str, args)), cwd=cwd, check=True)


def download(url, path, expected):
    if not path.exists():
        temporary = path.with_suffix(path.suffix + '.download')
        # Interrupted downloads are preserved for diagnosis, never accepted.
        with temporary.open('xb') as output, urllib.request.urlopen(url) as response:
            shutil.copyfileobj(response, output)
        if digest(temporary) != expected:
            raise RuntimeError(f'Download fingerprint mismatch: {temporary}')
        temporary.rename(path)
    if digest(path) != expected:
        raise RuntimeError(f'Cached input differs; preserved: {path}')


def head(path):
    return subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=path, text=True).strip()


def checkout(path, url, commit):
    if not path.exists():
        path.mkdir()
        run('git', 'init', path)
        run('git', 'remote', 'add', 'origin', url, cwd=path)
        run('git', 'fetch', '--depth=1', 'origin', commit, cwd=path)
        run('git', 'checkout', '--detach', commit, cwd=path)
    if head(path) != commit:
        raise RuntimeError(f'Unexpected checkout revision; preserved: {path}')


def apply(patch, target):
    if subprocess.run(['git', 'apply', '--check', str(patch)], cwd=target,
                      capture_output=True).returncode == 0:
        run('git', 'apply', patch, cwd=target)
    else:
        run('git', 'apply', '--reverse', '--check', patch, cwd=target)


def verify_sources(source):
    for relative, expected in LOCK['changedFiles'].items():
        if digest(source / relative) != expected:
            raise RuntimeError(f'Patched source differs from lock: {relative}')
    # A matching HEAD alone does not prove a reused checkout is the locked build.
    for repo, prefix in [(source, ''), (source / 'build', 'build/')]:
        changed = subprocess.check_output(['git', 'diff', '--name-only', 'HEAD'], cwd=repo,
                                          text=True).splitlines()
        unexpected = [prefix + name for name in changed
                      if prefix + name not in LOCK['changedFiles']]
        if unexpected:
            raise RuntimeError(f'Unexpected source changes; preserved: {unexpected}')
    for name, expected in LOCK['dependencies'].items():
        path = source / name
        if head(path) != expected:
            raise RuntimeError(f'Dependency revision differs: {name}')
        if name != 'build':
            changed = subprocess.check_output(['git', 'diff', '--name-only', 'HEAD'], cwd=path, text=True).strip()
            if changed:
                raise RuntimeError(f'Dependency has local changes; preserved: {name}')
    for name, expected in LOCK['tools'].items():
        if digest(source / name) != expected:
            raise RuntimeError(f'Build tool differs from lock: {name}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--vs', required=True)
    args = parser.parse_args()
    root, destination = args.root, args.output.resolve()
    if destination.exists():
        raise RuntimeError(f'Preserve existing candidate: {destination}')
    if digest(WORKSPACE / LOCK['patch']) != LOCK['patchSha256']:
        raise RuntimeError('Folio patch differs from lock')
    if digest(WORKSPACE / LOCK['advancePatch']) != LOCK['advancePatchSha256']:
        raise RuntimeError('Folio horizontal-advance patch differs from lock')
    if digest(WORKSPACE / LOCK['colorPatch']) != LOCK['colorPatchSha256']:
        raise RuntimeError('Folio default-color patch differs from lock')
    if digest(WORKSPACE / LOCK['formFontPatch']) != LOCK['formFontPatchSha256']:
        raise RuntimeError('Folio embedded form-font patch differs from lock')
    if digest(WORKSPACE / LOCK['borderPatch']) != LOCK['borderPatchSha256']:
        raise RuntimeError('Folio fractional form-border patch differs from lock')
    if digest(WORKSPACE / LOCK['separatorPatch']) != LOCK['separatorPatchSha256']:
        raise RuntimeError('Folio form-separator coordinate patch differs from lock')
    if digest(WORKSPACE / 'scripts/probe-pdfium-form-separators.py') != LOCK['separatorProbeSha256']:
        raise RuntimeError('Folio form-separator probe differs from lock')
    for path, expected in LOCK['formFontFixtures'].items():
        if digest(WORKSPACE / path) != expected:
            raise RuntimeError(f'Form-font fixture differs from lock: {path}')
    for path, expected in LOCK['advanceFixtures'].items():
        if digest(WORKSPACE / path) != expected:
            raise RuntimeError(f'Font-advance fixture differs from lock: {path}')
    # Fail early on missing probe dependencies, before downloading the toolchain.
    import fitz, PIL, pypdf  # noqa: F401
    os.environ.update(DEPOT_TOOLS_UPDATE='0', DEPOT_TOOLS_WIN_TOOLCHAIN='0',
                      PYTHONUTF8='1', GIT_CONFIG_COUNT='2',
                      GIT_CONFIG_KEY_0='core.longpaths', GIT_CONFIG_VALUE_0='true',
                      GIT_CONFIG_KEY_1='core.autocrlf', GIT_CONFIG_VALUE_1='false',
                      vs2022_install=args.vs,
                      WINDOWSSDKDIR=str(root / 'toolchain/Windows Kits/10'),
                      GYP_MSVS_OVERRIDE_PATH=str(root / 'toolchain'))
    os.environ['PATH'] = str(root / 'depot_tools') + os.pathsep + str(root / 'sdk/bin/10.0.28000.0/x64') + os.pathsep + os.environ['PATH']
    for name in ('depot_tools', 'distributor'):
        record = LOCK['repositories'][name]
        checkout(root / name, record['url'], record['commit'])
    for name, record in LOCK['downloads'].items():
        download(record['url'], root / name, record['sha256'])
    for name in LOCK['downloads']:
        if name.endswith('.nupkg'):
            run(root / 'nuget.exe', 'verify', '-Signatures', root / name)
    run(sys.executable, WORKSPACE / 'scripts/prepare-pdfium-sdk.py', '--root', root)
    run('powershell.exe', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
        WORKSPACE / 'scripts/prepare-pdfium-toolchain.ps1', '-BuildRoot', root,
        '-VisualStudio', args.vs)
    checkout_root = root / 'checkout'
    checkout_root.mkdir(exist_ok=True)
    config = 'solutions = [{"name": "pdfium", "url": "https://pdfium.googlesource.com/pdfium.git", "managed": False, "custom_deps": {}, "custom_vars": {"checkout_configuration": "minimal"}}]\ntarget_os = ["win"]\n'
    config_path = checkout_root / '.gclient'
    if config_path.exists():
        # Existing gclient files may differ only in formatting; never overwrite them.
        def config_values(text):
            values = {}
            for node in ast.parse(text).body:
                if not isinstance(node, ast.Assign) or len(node.targets) != 1 or not isinstance(node.targets[0], ast.Name):
                    raise RuntimeError('Unexpected executable gclient configuration')
                values[node.targets[0].id] = ast.literal_eval(node.value)
            return values
        if config_values(config_path.read_text()) != config_values(config):
            raise RuntimeError('Existing gclient configuration differs from the locked configuration')
    else:
        config_path.write_text(config, encoding='utf-8')
    source = checkout_root / 'pdfium'
    if not (source / 'buildtools/win/gn.exe').exists():
        run(root / 'depot_tools/gclient.bat', 'sync', '--revision',
            'pdfium@' + LOCK['repositories']['pdfium']['commit'],
            '--no-history', '--shallow', '--nohooks', cwd=checkout_root)
        run(root / 'depot_tools/gclient.bat', 'runhooks', cwd=checkout_root)
    for name, path in [('pdfium', source), ('build', source / 'build')]:
        if head(path) != LOCK['repositories'][name]['commit']:
            raise RuntimeError(f'Unexpected {name} revision')
    distributor = root / 'distributor'
    # Distributor patches overlap. Reverse-checking each against the final tree
    # is invalid; recognize the complete accepted tree by its locked file hashes.
    distributor_applied = all((source / name).is_file() and digest(source / name) == expected
                              for name, expected in LOCK['distributorChangedFiles'].items())
    if not distributor_applied:
        for name, target in [('shared_library.patch', source), ('public_headers.patch', source),
                             ('clang_rt.patch', source / 'build'), ('win/build.patch', source / 'build')]:
            apply(distributor / 'patches' / name, target)
    apply(WORKSPACE / LOCK['patch'], source)
    apply(WORKSPACE / LOCK['advancePatch'], source)
    apply(WORKSPACE / LOCK['colorPatch'], source)
    apply(WORKSPACE / LOCK['formFontPatch'], source)
    apply(WORKSPACE / LOCK['borderPatch'], source)
    apply(WORKSPACE / LOCK['separatorPatch'], source)
    resources = (distributor / 'patches/win/resources.rc').read_text(encoding='utf-8')
    for key, value in [('YEAR', '2026'), ('VERSION_CSV', '155,0,8044,0'), ('VERSION', '155.0.8044.0')]:
        resources = resources.replace('${' + key + '}', value).replace('$' + key, value)
    (source / 'resources.rc').write_text(resources, encoding='utf-8')
    verify_sources(source)
    output = source / 'out/folio-baseline'  # Reuse the accepted baseline/candidate cache.
    output.mkdir(parents=True, exist_ok=True)
    (output / 'args.gn').write_text(''.join(f'{key} = {json.dumps(value)}\n' for key, value in LOCK['gnArgs'].items()), encoding='utf-8')
    run(source / 'buildtools/win/gn.exe', 'gen', 'out/folio-baseline', cwd=source)
    run('ninja.exe', '-C', output, '-j', '4', 'pdfium')
    verify_sources(source)
    destination.mkdir(parents=True)
    shutil.copyfile(output / 'pdfium.dll', destination / 'pdfium.dll')
    shutil.copyfile(output / 'args.gn', destination / 'args.gn')
    # Same pinned distributor/component configuration; preserve its complete notices.
    archive = root / 'pdfium-win-x64-8044.tgz'
    notices = root / 'distributor-notices'
    if not notices.exists():
        notices.mkdir()
        run('tar', '-xzf', archive, '-C', notices)
    for name in ('LICENSE', 'VERSION'):
        shutil.copyfile(notices / name, destination / name)
    shutil.copytree(notices / 'licenses', destination / 'licenses')
    probe = destination / 'inheritance-probe'
    run(sys.executable, WORKSPACE / 'scripts/probe-pdfium-form-inheritance.py',
        '--dll', destination / 'pdfium.dll', '--output', probe)
    run(sys.executable, WORKSPACE / 'scripts/probe-pdfium-font-advances.py', '--extended',
        '--dll', destination / 'pdfium.dll', '--output', destination / 'advance-probe')
    run(sys.executable, WORKSPACE / 'scripts/probe-pdfium-allcaps.py',
        '--dll', destination / 'pdfium.dll', '--output', destination / 'allcaps-probe')
    run(sys.executable, WORKSPACE / 'scripts/probe-pdfium-form-colors.py',
        '--dll', destination / 'pdfium.dll', '--output', destination / 'color-probe')
    run(sys.executable, WORKSPACE / 'scripts/probe-pdfium-form-fonts.py',
        '--dll', destination / 'pdfium.dll', '--output', destination / 'form-font-probe')
    run(sys.executable, WORKSPACE / 'scripts/probe-pdfium-form-borders.py',
        '--dll', destination / 'pdfium.dll', '--output', destination / 'form-border-probe')
    run(sys.executable, WORKSPACE / 'scripts/probe-pdfium-form-border-limits.py',
        '--dll', destination / 'pdfium.dll', '--output', destination / 'form-border-limits-probe')
    baseline = WORKSPACE / 'src-tauri/resources/pdfium/pdfium.dll'
    baseline_args = ['--baseline-dll', baseline] if baseline.is_file() else []
    run(sys.executable, WORKSPACE / 'scripts/probe-pdfium-form-separators.py',
        '--dll', destination / 'pdfium.dll', '--output', destination / 'form-separator-probe',
        *baseline_args)
    provenance = dict(project='PDFium', distributor='Folio local source build',
                      release='chromium/8044', variant=LOCK['variant'],
                      sourceCommit=LOCK['repositories']['pdfium']['commit'],
                      buildCommit=LOCK['repositories']['build']['commit'],
                      distributorCommit=LOCK['repositories']['distributor']['commit'],
                      depotToolsCommit=LOCK['repositories']['depot_tools']['commit'],
                      patchSha256=LOCK['patchSha256'], dllSha256=digest(destination / 'pdfium.dll'),
                      advancePatchSha256=LOCK['advancePatchSha256'],
                      colorPatchSha256=LOCK['colorPatchSha256'],
                      formFontPatchSha256=LOCK['formFontPatchSha256'],
                      borderPatchSha256=LOCK['borderPatchSha256'],
                      separatorPatchSha256=LOCK['separatorPatchSha256'],
                      separatorProbeSha256=LOCK['separatorProbeSha256'],
                      sourceLockSha256=digest(WORKSPACE / 'scripts/pdfium-source-lock.json'),
                      sdkPackageVersion='10.0.28000.2270', msvcVersion='14.44.35207')
    (destination / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n', encoding='utf-8')
    print(f'Candidate built and direct probe passed: {destination}', flush=True)
    print('Run native and desktop acceptance before release. See docs/building-pdfium.md.')


if __name__ == '__main__':
    main()
