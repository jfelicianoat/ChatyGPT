"""Contrato entre la interfaz y las órdenes de Tauri.

Estas comprobaciones cubren un hueco que ninguna otra prueba alcanza. Las
órdenes de Tauri se enlazan **por cadena de texto**: el frontend escribe
`invoke("delete_memory_item", { memoryId })` y Rust declara
`fn delete_memory_item(memory_id: String, ...)`. No hay tipos compartidos entre
ambos lados, así que un nombre mal escrito, una orden que se olvida de registrar
o un argumento renombrado en un solo lado compilan sin protestar y fallan
únicamente al ejecutar la aplicación y pulsar ese botón concreto.

Al no existir pruebas end-to-end, ese fallo llegaría hasta el usuario. Estas
pruebas lo convierten en un fallo de CI leyendo ambas fuentes y comparándolas.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LIB_RS = ROOT / "apps" / "desktop" / "src-tauri" / "src" / "lib.rs"
PLATFORM_TS = ROOT / "apps" / "desktop" / "src" / "platform.ts"
APP_TSX = ROOT / "apps" / "desktop" / "src" / "App.tsx"

# Tipos que Tauri inyecta al invocar: no viajan desde el frontend.
INJECTED_TYPES = ("State<", "AppHandle", "Window", "WebviewWindow")


def to_snake_case(name: str) -> str:
    """`memoryId` -> `memory_id`, la conversión que aplica Tauri."""
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def balanced_block(text: str, opening_index: int, opening: str, closing: str) -> str:
    """Contenido entre delimitadores equilibrados a partir de `opening_index`."""
    depth = 0
    for index in range(opening_index, len(text)):
        if text[index] == opening:
            depth += 1
        elif text[index] == closing:
            depth -= 1
            if depth == 0:
                return text[opening_index + 1 : index]
    raise AssertionError("delimitador sin cerrar al analizar el contrato")


def split_top_level(text: str, separator: str = ",") -> list[str]:
    """Divide por comas ignorando las que van dentro de `<>`, `()`, `[]` o `{}`.

    Es necesario porque `state: State<'_, AppState>` contiene una coma que no
    separa parámetros.
    """
    parts: list[str] = []
    depth = 0
    current: list[str] = []
    for character in text:
        if character in "<([{":
            depth += 1
        elif character in ">)]}":
            depth -= 1
        if character == separator and depth == 0:
            parts.append("".join(current))
            current = []
        else:
            current.append(character)
    parts.append("".join(current))
    return [part.strip() for part in parts if part.strip()]


def declared_commands() -> dict[str, set[str]]:
    """Órdenes declaradas en Rust con los argumentos que espera del frontend."""
    source = LIB_RS.read_text(encoding="utf-8")
    commands: dict[str, set[str]] = {}
    pattern = re.compile(
        r"#\[tauri::command\]\s*(?:#\[[^\]]*\]\s*)*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*\("
    )
    for match in pattern.finditer(source):
        raw_params = balanced_block(source, match.end() - 1, "(", ")")
        arguments = set()
        for raw in split_top_level(raw_params):
            name, _, declared_type = raw.partition(":")
            if any(injected in declared_type for injected in INJECTED_TYPES):
                continue
            if name.strip():
                arguments.add(name.strip())
        commands[match.group(1)] = arguments
    return commands


def registered_commands() -> set[str]:
    """Órdenes incluidas en `generate_handler!`, las únicas invocables."""
    source = LIB_RS.read_text(encoding="utf-8")
    start = source.index("generate_handler![")
    block = balanced_block(source, source.index("[", start), "[", "]")
    return {line.strip().rstrip(",") for line in block.splitlines() if line.strip().rstrip(",")}


def invoked_commands() -> dict[str, set[str]]:
    """Órdenes invocadas desde `platform.ts` con los argumentos que envía."""
    source = PLATFORM_TS.read_text(encoding="utf-8")
    invocations: dict[str, set[str]] = {}
    for match in re.finditer(r'invoke<[^>]*>\(\s*"(\w+)"', source):
        rest = source[match.end() :]
        arguments: set[str] = set()
        if rest.lstrip().startswith(","):
            after_comma = rest.index(",") + 1
            brace = rest.find("{", after_comma)
            paren = rest.find(")", after_comma)
            if brace != -1 and (paren == -1 or brace < paren):
                for entry in split_top_level(balanced_block(rest, brace, "{", "}")):
                    key = entry.split(":")[0].strip()
                    if key:
                        arguments.add(key)
        invocations[match.group(1)] = arguments
    return invocations


class TauriCommandContractTests(unittest.TestCase):
    def test_every_declared_command_is_registered(self) -> None:
        """Una orden declarada y no registrada es código muerto.

        Compila y parece implementada, pero `invoke` la rechaza en ejecución.
        """
        missing = sorted(set(declared_commands()) - registered_commands())
        self.assertEqual(
            [],
            missing,
            f"órdenes declaradas sin registrar en generate_handler!: {missing}",
        )

    def test_every_registered_command_exists(self) -> None:
        """Registrar un nombre inexistente impide compilar; se comprueba igual
        para que el registro y las declaraciones no se separen."""
        orphans = sorted(registered_commands() - set(declared_commands()))
        self.assertEqual([], orphans, f"órdenes registradas sin declarar: {orphans}")

    def test_every_invoked_command_exists_and_is_registered(self) -> None:
        """El error más caro: la interfaz llama a algo que no existe.

        No lo detecta ni TypeScript ni el compilador de Rust; solo aparece al
        pulsar ese botón concreto en la aplicación en marcha.
        """
        declared = set(declared_commands())
        registered = registered_commands()
        invoked = set(invoked_commands())
        self.assertEqual(
            [], sorted(invoked - declared), "la interfaz invoca órdenes que no existen"
        )
        self.assertEqual(
            [],
            sorted(invoked - registered),
            "la interfaz invoca órdenes que no están registradas",
        )

    def test_arguments_match_after_the_case_conversion_tauri_applies(self) -> None:
        """Tauri traduce `memoryId` a `memory_id`; un renombrado en un solo lado
        provoca un fallo silencioso hasta que se ejecuta esa acción."""
        declared = declared_commands()
        mismatches = []
        for command, sent in sorted(invoked_commands().items()):
            expected = declared.get(command)
            if expected is None:
                continue
            converted = {to_snake_case(argument) for argument in sent}
            if converted != expected:
                mismatches.append(
                    f"{command}: la interfaz envía {sorted(converted)} y Rust espera {sorted(expected)}"
                )
        self.assertEqual([], mismatches, "\n".join(mismatches))

    def test_no_command_is_unreachable_from_the_interface(self) -> None:
        """Una orden que nadie invoca es superficie sin usar: o falta cablearla
        en la interfaz, o sobra en el backend. Ambas cosas conviene saberlas."""
        unreachable = sorted(set(declared_commands()) - set(invoked_commands()))
        self.assertEqual(
            [], unreachable, f"órdenes que la interfaz nunca invoca: {unreachable}"
        )


class DestructiveConfirmationTests(unittest.TestCase):
    """Ninguna orden debe recibir una confirmación que nadie dio.

    Varias órdenes de Rust exigen `confirmed = true` antes de actuar. Si
    `platform.ts` lo fija por su cuenta y quien llama no pregunta, esa
    comprobación deja de proteger nada: es una confirmación afirmada, no
    obtenida. El encargo trata la ejecución de acciones sensibles sin
    confirmación como un defecto invalidante, así que se comprueba de forma
    estructural en lugar de confiar en la revisión manual.
    """

    #: Señales de que la persona ha decidido antes de llamar.
    #:
    #: Se comparan sin distinguir mayúsculas para reconocer tanto
    #: `window.confirm(...)` como una casilla de estado del formulario
    #: (`scheduleConfirmed`), que son las dos formas que usa la aplicación.
    CONFIRMATION_MARKERS = ("confirm", "opendialog(", "dialog.")

    def platform_methods_that_assert_confirmation(self) -> list[str]:
        source = PLATFORM_TS.read_text(encoding="utf-8")
        methods = []
        for match in re.finditer(r"^  (\w+)\(", source, re.M):
            body_end = source.find("\n  },", match.start())
            body = source[match.start() : body_end if body_end != -1 else len(source)]
            if re.search(r"confirmed:\s*(true|enabled)", body):
                methods.append(match.group(1))
        return methods

    def enclosing_function_body(self, source: str, position: int) -> str:
        """Cuerpo de la función de `App.tsx` que contiene `position`.

        Se busca hacia atrás la declaración más cercana y se devuelve su bloque
        equilibrado: comprobar dentro de la función es mucho más fiable que
        mirar una ventana de caracteres alrededor de la llamada.
        """
        declaration = None
        for match in re.finditer(r"(?:const \w+ = (?:async )?\([^)]*\) =>|function \w+\()", source):
            if match.start() > position:
                break
            declaration = match
        if declaration is None:
            return source[max(0, position - 2000) : position]
        brace = source.find("{", declaration.end() - 1)
        if brace == -1 or brace > position:
            return source[max(0, position - 2000) : position]
        return balanced_block(source, brace, "{", "}")

    def test_confirmation_is_asked_before_it_is_asserted(self) -> None:
        app_source = APP_TSX.read_text(encoding="utf-8")
        unconfirmed = []
        for method in self.platform_methods_that_assert_confirmation():
            call_sites = [
                match.start()
                for match in re.finditer(rf"platform\.{method}\(", app_source)
            ]
            self.assertTrue(
                call_sites,
                f"platform.{method} afirma una confirmación pero nadie lo llama",
            )
            for position in call_sites:
                body = self.enclosing_function_body(app_source, position).lower()
                if not any(marker in body for marker in self.CONFIRMATION_MARKERS):
                    unconfirmed.append(method)
        self.assertEqual(
            [],
            sorted(set(unconfirmed)),
            "estas acciones envían confirmed sin preguntar a la persona: "
            f"{sorted(set(unconfirmed))}",
        )


if __name__ == "__main__":
    unittest.main()
