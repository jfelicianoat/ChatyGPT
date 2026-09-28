from pathlib import Path
import re

root = Path(__file__).resolve().parents[3]
report = Path(__file__).resolve().parents[1] / 'INFORME.md'
text = report.read_text(encoding='utf-8')

def link(match):
    reference = match.group(1)
    parts = reference.rsplit(':', 1)
    line = int(parts[1]) if len(parts) == 2 and parts[1].isdigit() else None
    relative = parts[0] if line else reference
    path = root / relative
    assert path.is_file(), reference
    if line:
        assert line <= len(path.read_text(encoding='utf-8').splitlines()), reference
    target = path.as_posix() + (f':{line}' if line else '')
    return f'[{reference}](<{target}>)'

text = re.sub(r'\[\[(.*?)\]\]', link, text)
text = text.replace('https://docs.rs/reqwest/0.12.28/reqwest/', 'https://docs.rs/reqwest/latest/reqwest/')
report.write_text(text, encoding='utf-8')
print('Informe:', report)
print('Palabras:', len(text.split()))
print('Hallazgos:', len(re.findall(r'^### H\d+', text, re.M)))
print('Enlaces locales:', len(re.findall(r'\]\(<[A-Z]:/', text)))
