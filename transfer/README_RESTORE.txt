КАК ВОССТАНОВИТЬ ФАЙЛ MouseTweaks-RS_v1.0.0.hmod ИЗ ЧАТЕЙ

1. Соберите ВСЕ чанки base64 из сообщений в один файл all_b64.txt
   (склейте содержимое блоков подряд, БЕЗ переносов строк между чанками;
    переносы внутри чанков на 100 символов можно убрать командой tr -d '\n\r').

2. Windows (PowerShell):
     certutil -decode all_b64_clean.txt MouseTweaks-RS_v1.0.0.hmod
   Linux/macOS:
     base64 -d all_b64_clean.txt > MouseTweaks-RS_v1.0.0.hmod

3. Проверьте целостность (файл ДОЛЖЕН совпасть):
     размер  = 594815 байт
     md5     = 5b437f7ff849f87ab60bc513c048d788
     sha256  = 7b619bc4a08342730268d8dc473c104f44178bf42d8c96c661901361aff51298
   PowerShell: Get-FileHash -Algorithm SHA256 MouseTweaks-RS_v1.0.0.hmod

Всего чанков: 32. Каждый помечен как [CHUNK i/N]. Порядок склейки — строго по номеру.
