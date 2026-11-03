"""Aggiunge la correzione geografica del meteo ai changelog 0.9.20.

La correzione (commit eaa95d1) precede l'aggiornamento delle guide:
nel changelog diventa la voce 1, mentre le guide diventano la voce 2.
Conserva i byte precedenti, la codifica e le terminazioni di riga.
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HEADING = 'Version 0.9.20 – 2026-10-25'
NOTES = {
    'CHANGELOG.md': 'Weather: improved location labels in city search results and recently used cities. Where available, county or district, region and country are now shown to distinguish places with the same name. Repeated location names are omitted, and previously saved cities remain compatible.',
    'CHANGELOG_IT.md': 'Meteo: migliorate le indicazioni geografiche nei risultati di ricerca e nelle città recenti. Quando disponibili, vengono mostrati anche contea o distretto, regione e Paese, così da distinguere località con lo stesso nome. Eliminati i nomi ripetuti e mantenuta la compatibilità con le città già salvate.',
    'CHANGELOG_CS.md': 'Počasí: zpřesněny názvy lokalit ve výsledcích hledání a seznamu nedávno použitých měst. Jsou-li dostupné, zobrazují se také okres nebo správní obvod, region a země, aby bylo možné rozlišit stejnojmenná místa. Duplicitní názvy se vynechávají a dříve uložená města zůstávají kompatibilní.',
    'CHANGELOG_DE.md': 'Wetter: Ortsangaben in den Suchergebnissen und zuletzt verwendeten Städten verbessert. Sofern verfügbar, werden auch Landkreis oder Bezirk, Region und Land angezeigt, damit gleichnamige Orte unterschieden werden können. Doppelte Ortsangaben werden vermieden; bereits gespeicherte Städte bleiben kompatibel.',
    'CHANGELOG_ES.md': 'Tiempo: mejoradas las etiquetas de ubicación en los resultados de búsqueda y las ciudades recientes. Cuando están disponibles, se muestran también el condado o distrito, la región y el país para distinguir localidades con el mismo nombre. Se evitan nombres repetidos y se mantiene la compatibilidad con las ciudades guardadas anteriormente.',
    'CHANGELOG_FR.md': 'Météo : amélioration des indications géographiques dans les résultats de recherche et les villes récentes. Le comté ou district, la région et le pays s’affichent lorsqu’ils sont disponibles afin de distinguer les lieux portant le même nom. Les informations répétées sont supprimées et les villes déjà enregistrées restent compatibles.',
    'CHANGELOG_ID.md': 'Cuaca: label lokasi pada hasil pencarian kota dan daftar kota terbaru diperjelas. Jika tersedia, kabupaten atau distrik, wilayah, dan negara kini ditampilkan agar lokasi bernama sama dapat dibedakan. Nama lokasi yang berulang dihilangkan dan kota yang sudah tersimpan tetap kompatibel.',
    'CHANGELOG_LT.md': 'Orų prognozė: patikslintos vietovių žymos miestų paieškos rezultatuose ir neseniai naudotų miestų sąraše. Kai yra duomenų, rodomi rajonas ar apygarda, regionas ir šalis, kad būtų galima atskirti vienodai pavadintas vietoves. Pasikartojantys pavadinimai pašalinti, o anksčiau išsaugoti miestai išlieka suderinami.',
    'CHANGELOG_PL.md': 'Pogoda: ulepszono opisy lokalizacji w wynikach wyszukiwania i na liście ostatnich miast. Gdy dane są dostępne, pokazywane są również powiat lub okręg, region i kraj, co pozwala odróżnić miejscowości o tej samej nazwie. Usunięto powtarzające się nazwy, zachowując zgodność z wcześniej zapisanymi miastami.',
    'CHANGELOG_PT.md': 'Meteorologia: melhoradas as indicações de localização nos resultados de pesquisa e nas cidades recentes. Quando disponíveis, são apresentados também o distrito ou concelho, a região e o país para distinguir localidades com o mesmo nome. Evitam-se nomes repetidos e mantém-se a compatibilidade com as cidades já guardadas.',
    'CHANGELOG_PT_BR.md': 'Previsão do tempo: melhoradas as informações de localização nos resultados de busca e nas cidades recentes. Quando disponíveis, são exibidos também condado ou distrito, região e país para diferenciar localidades com o mesmo nome. Nomes repetidos são eliminados e as cidades salvas anteriormente continuam compatíveis.',
    'CHANGELOG_RU.md': 'Погода: улучшены обозначения местоположений в результатах поиска и списке недавно выбранных городов. При наличии данных теперь отображаются также округ или район, регион и страна, чтобы различать одноимённые населённые пункты. Повторяющиеся названия исключаются, а ранее сохранённые города остаются совместимыми.',
    'CHANGELOG_SR.md': 'Vremenska prognoza: poboljšane su oznake lokacija u rezultatima pretrage i listi nedavno korišćenih gradova. Kada su dostupni, prikazuju se i okrug ili distrikt, region i država radi razlikovanja mesta sa istim imenom. Duplirani nazivi se uklanjaju, a ranije sačuvani gradovi ostaju kompatibilni.',
    'CHANGELOG_SV.md': 'Väder: platsangivelserna har förbättrats i sökresultat och listan över senast använda städer. När uppgifterna finns visas även län eller distrikt, region och land för att skilja orter med samma namn åt. Upprepade platsnamn tas bort och tidigare sparade städer fungerar fortfarande.',
    'CHANGELOG_UK.md': 'Погода: покращено позначення місцевостей у результатах пошуку та списку нещодавно використаних міст. За наявності даних тепер також показуються округ або район, регіон і країна, щоб розрізняти однойменні населені пункти. Повторювані назви прибрано, а раніше збережені міста залишаються сумісними.',
    'CHANGELOG_VI.md': 'Thời tiết: cải thiện nhãn vị trí trong kết quả tìm kiếm và danh sách thành phố gần đây. Khi có dữ liệu, chương trình hiển thị thêm quận hoặc huyện, vùng và quốc gia để phân biệt những địa điểm trùng tên. Tên địa danh lặp lại được loại bỏ và các thành phố đã lưu trước đây vẫn tương thích.',
    'CHANGELOG_ZH.md': '天气：改进城市搜索结果和最近使用城市列表中的地点标签。数据可用时会同时显示县或区、地区及国家，以区分同名地点；自动去除重复的地名，并保持与此前保存的城市数据兼容。',
    'CHANGELOG-HI.txt': 'मौसम: शहर खोज परिणामों और हाल में उपयोग किए गए शहरों में स्थान की जानकारी बेहतर की गई है। उपलब्ध होने पर काउंटी या ज़िला, क्षेत्र और देश भी दिखते हैं, ताकि एक जैसे नाम वाले स्थान पहचाने जा सकें। दोहराए गए स्थान नाम हटाए जाते हैं और पहले से सहेजे गए शहर भी काम करते रहेंगे।',
}


def main():
    changelogs = {p.name for p in ROOT.glob('CHANGELOG*') if p.is_file()}
    if changelogs != set(NOTES):
        raise RuntimeError(f'Elenco changelog inatteso: {sorted(changelogs ^ set(NOTES))}')
    modifications = []
    for filename, description in NOTES.items():
        path = ROOT / filename
        original = path.read_bytes()
        eol = b'\r\n' if b'\r\n' in original else b'\n'
        intro = HEADING.encode('utf-8') + eol + eol + b'1. '
        if original.count(intro) != 1:
            raise RuntimeError(f'Intestazione della versione non trovata una sola volta: {filename}')
        beginning = original.index(intro) + len(intro)
        end = original.find(eol, beginning)
        if end < 0:
            raise RuntimeError(f'Voce della guida non trovata: {filename}')
        existing = original[beginning:end].decode('utf-8')
        if 'SonarMusic' not in existing:
            raise RuntimeError(f'Prima voce non relativa alle guide: {filename}')
        weather = b'1. ' + description.encode('utf-8') + eol + b'2. '
        replacement = HEADING.encode('utf-8') + eol + eol + weather
        updated = original.replace(intro, replacement, 1)
        if updated == original:
            raise RuntimeError(f'Nessuna modifica: {filename}')
        modifications.append((path, original, updated))
    for path, original, updated in modifications:
        path.write_bytes(updated)
        print(f'Aggiornato: {path.name}')
    print(f'Totale: {len(modifications)} changelog. Meteo voce 1, guide voce 2.')


if __name__ == '__main__':
    main()
