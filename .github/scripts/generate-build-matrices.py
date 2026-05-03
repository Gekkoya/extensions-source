#!/usr/bin/env python3
"""
Script para generar matrices de build detectando cambios en el repositorio.
"""

import itertools
import json
import os
import re
import subprocess
import sys
from pathlib import Path

EXTENSION_REGEX = re.compile(r"^src/(?P<lang>\w+)/(?P<extension>\w+)")
MULTISRC_LIB_REGEX = re.compile(r"^lib-multisrc/(?P<multisrc>\w+)")
LIB_REGEX = re.compile(r"^lib/(?P<lib>\w+)")
CORE_FILES_REGEX = re.compile(
    r"^(core/|lib/|Cargo\.toml|rust-toolchain\.toml|\.cargo/|\.github/scripts)"
)


def run_command(command: str) -> str:
    """Ejecuta un comando y retorna su output"""
    result = subprocess.run(command, capture_output=True, text=True, shell=True)
    if result.returncode != 0:
        print(result.stderr.strip(), file=sys.stderr)
        sys.exit(result.returncode)
    return result.stdout.strip()


def get_cargo_package_name(extension_path: Path) -> str:
    """Extrae el nombre del package desde Cargo.toml"""
    cargo_toml = extension_path / "Cargo.toml"
    if not cargo_toml.exists():
        return extension_path.name
    
    with open(cargo_toml, "r", encoding="utf-8") as f:
        for line in f:
            if line.strip().startswith("name"):
                # name = "mangasin"
                match = re.search(r'name\s*=\s*"([^"]+)"', line)
                if match:
                    return match.group(1)
    
    return extension_path.name


def resolve_dependent_libs(libs: set[str]) -> set[str]:
    """
    Retorna todas las libs que dependen de las libs pasadas,
    resolviendo dependencias transitivas recursivamente
    """
    if not libs:
        return set()

    all_dependent_libs = set()
    to_process = set(libs)

    while to_process:
        current_libs = to_process
        to_process = set()

        # Buscar en Cargo.toml de cada lib
        lib_dependency = re.compile(
            rf'({"|".join(map(re.escape, current_libs))})\s*=\s*\{{\s*path'
        )

        for lib in Path("lib").iterdir():
            if not lib.is_dir() or lib.name in all_dependent_libs or lib.name in libs:
                continue

            cargo_file = lib / "Cargo.toml"
            if not cargo_file.is_file():
                continue

            content = cargo_file.read_text("utf-8")

            if lib_dependency.search(content):
                all_dependent_libs.add(lib.name)
                to_process.add(lib.name)

    return all_dependent_libs


def resolve_multisrc_lib(libs: set[str]) -> set[str]:
    """
    Retorna todos los multisrc que dependen de las libs pasadas
    """
    if not libs:
        return set()

    lib_dependency = re.compile(
        rf'({"|".join(map(re.escape, libs))})\s*=\s*\{{\s*path'
    )

    multisrcs = set()

    for multisrc in Path("lib-multisrc").iterdir():
        if not multisrc.is_dir():
            continue
            
        cargo_file = multisrc / "Cargo.toml"
        if not cargo_file.is_file():
            continue

        content = cargo_file.read_text("utf-8")

        if lib_dependency.search(content):
            multisrcs.add(multisrc.name)

    return multisrcs


def resolve_ext(multisrcs: set[str], libs: set[str]) -> set[tuple[str, str]]:
    """
    Retorna todas las extensiones que dependen de los multisrcs o libs pasados
    """
    if not multisrcs and not libs:
        return set()

    patterns = []
    if multisrcs:
        multisrc_pattern = '|'.join(map(re.escape, multisrcs))
        patterns.append(rf'({multisrc_pattern})\s*=\s*\{{\s*path')
    if libs:
        lib_pattern = '|'.join(map(re.escape, libs))
        patterns.append(rf'({lib_pattern})\s*=\s*\{{\s*path')

    regex = re.compile('|'.join(patterns))

    extensions = set()

    for lang in Path("src").iterdir():
        if not lang.is_dir():
            continue
            
        for extension in lang.iterdir():
            if not extension.is_dir():
                continue
                
            cargo_file = extension / "Cargo.toml"
            if not cargo_file.is_file():
                continue

            content = cargo_file.read_text("utf-8")

            if regex.search(content):
                pkg_name = get_cargo_package_name(extension)
                extensions.add((lang.name, extension.name, pkg_name))

    return extensions


def get_module_list(ref: str) -> tuple[list[dict], list[str]]:
    """
    Detecta qué módulos necesitan recompilarse basándose en los cambios
    """
    diff_output = run_command(f"git diff --name-status {ref}").splitlines()

    changed_files = [
        file
        for line in diff_output
        for file in line.split("\t", 2)[1:]
    ]

    modules = []  # Lista de dicts con {lang, extension, package}
    multisrcs = set()
    libs = set()
    deleted = set()
    core_files_changed = False

    for file in map(lambda x: Path(x).as_posix(), changed_files):
        if CORE_FILES_REGEX.search(file):
            core_files_changed = True

        elif match := EXTENSION_REGEX.search(file):
            lang = match.group("lang")
            extension = match.group("extension")
            ext_path = Path("src", lang, extension)
            
            if ext_path.is_dir():
                pkg_name = get_cargo_package_name(ext_path)
                modules.append({
                    "lang": lang,
                    "extension": extension,
                    "package": pkg_name
                })
            
            deleted.add(f"{lang}.{extension}")

        elif match := MULTISRC_LIB_REGEX.search(file):
            multisrc = match.group("multisrc")
            if Path("lib-multisrc", multisrc).is_dir():
                multisrcs.add(multisrc)

        elif match := LIB_REGEX.search(file):
            lib = match.group("lib")
            if Path("lib", lib).is_dir():
                libs.add(lib)

    if core_files_changed:
        all_modules, all_deleted = get_all_modules()
        modules.extend(all_modules)
        deleted.update(all_deleted)
        return modules, list(deleted)

    # Resolver dependencias
    libs.update(resolve_dependent_libs(libs))
    multisrcs.update(resolve_multisrc_lib(libs))
    extensions = resolve_ext(multisrcs, libs)
    
    for lang, extension, pkg_name in extensions:
        modules.append({
            "lang": lang,
            "extension": extension,
            "package": pkg_name
        })
        deleted.add(f"{lang}.{extension}")

    return modules, list(deleted)


def get_all_modules() -> tuple[list[dict], list[str]]:
    """Retorna todos los módulos del workspace"""
    modules = []
    deleted = []
    
    for lang in Path("src").iterdir():
        if not lang.is_dir():
            continue
            
        for extension in lang.iterdir():
            if not extension.is_dir():
                continue
                
            pkg_name = get_cargo_package_name(extension)
            modules.append({
                "lang": lang.name,
                "extension": extension.name,
                "package": pkg_name
            })
            deleted.append(f"{lang.name}.{extension.name}")
    
    return modules, deleted


def main() -> None:
    if len(sys.argv) < 2:
        print("Usage: generate-build-matrices.py <ref>", file=sys.stderr)
        sys.exit(1)
    
    _, ref = sys.argv
    modules, deleted = get_module_list(ref)

    # Agrupar en chunks para paralelizar
    chunk_size = int(os.getenv("CI_CHUNK_SIZE", "10"))
    
    chunked = {
        "chunk": [
            {"number": i + 1, "modules": list(chunk)}
            for i, chunk in enumerate(itertools.batched(modules, chunk_size))
        ]
    }

    print(f"Module chunks to build:\n{json.dumps(chunked, indent=2)}\n")
    print(f"Modules to delete:\n{json.dumps(deleted, indent=2)}")

    # Output para GitHub Actions
    if os.getenv("CI") == "true":
        with open(os.getenv("GITHUB_OUTPUT"), 'a') as out_file:
            out_file.write(f"matrix={json.dumps(chunked)}\n")
            out_file.write(f"delete={json.dumps(deleted)}\n")


if __name__ == '__main__':
    main()
