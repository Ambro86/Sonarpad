# Přehled změn

Verze 0.9.8 – 2026-09-12

Zvukový popis s AI
1. Přidána konzervativní obnova exportu vícekanálového zvuku bez změny pipeline u souborů, které již fungují. Sonarpad nejprve stále používá původní rozložení kanálů a pouze v případě potřeby doplní neúplný poslední PCM rámec. Pokud vícekanálový mezilehlý WAV přesto selže, protože počet vzorků není násobkem počtu kanálů, Sonarpad automaticky zopakuje pouze tento export ve stereu. Úspěšné mono, stereo a vícekanálové exporty zůstávají beze změny.

Tmavý motiv a přístupnost
1. Opraven problém, kdy tmavý motiv narušoval nativní dialogy Windows a potvrzovací zprávy. Sonarpad již nepoužívá vlastní subclass tmavého motivu na systémové dialogové stromy `#32770`, takže dialogy Otevřít/Uložit, export diagnostiky a MessageBox zůstávají spravovány Windows a správně získávají fokus klávesnice/NVDA. Opravena byla také životnost výchozí přípony ZIP v dialogu Uložit diagnostiku.

Verze 0.9.7 – 2026-09-11

AI audiopopis
1. Přidána volitelná možnost vytvořit audiopopis v původním videosouboru. Je-li aktivní, Sonarpad nyní upřednostní MP4: zkopíruje původní video stream bez překódování a vloží smíchanou stopu audiopopisu. Pokud FFmpeg při vytváření MP4 oznámí nekompatibilitu kontejneru nebo zápisu paketů, Sonarpad automaticky zopakuje stejný export do MKV, stále bez překódování videa. MKV lze také zvolit ručně. Rozšířené pauzy se v tomto režimu ignorují, aby zůstaly zvuk a video synchronizované.

2. Vylepšena opětovná analýza segmentů v uložených projektech audiopopisu. Okno nyní používá přístup k AI již nastavený v hlavním okně Vytvořit audiopopis, takže již neduplikuje klíč API, kredit Sonarpad AI ani výběr modelu. Znovu analyzovaný segment nyní zachovává celou skupinu popisů projektu patřících do stejného analytického úseku místo pouze prvního nebo nejbližšího popisu; všechny popisy ve skupině jsou označeny jako znovu analyzované a „Použít segment a znovu exportovat MP3“ použije celou skupinu najednou. Náhledy rozšířených pauz se nyní syntetizují přímo místo hledání pozice v exportovaném MP3, takže náhled nezačne zvukem filmu ani neusekne popis.

3. Konzervativní záložní postup pro média Gemini byl rozšířen bez změny stávajícího funkčního postupu. HTTP 400 `INVALID_ARGUMENT` nadále spouští menší bloky MKV (přibližně 15 MB) a poté kompatibilní bloky MP4. Pokud Gemini přijatý blok zpracuje do stavu `FAILED`, Sonarpad nahraje přesně tentýž blok ještě jednou a teprve potom přejde na MP4; opakování při serverovém Code 13 je omezeno na tři, aby nevznikalo nekonečné čekání. Chyby sítě, kvóty, ověření nebo syntézy tento postup nespouštějí.

4. Opravena trvalost přihlašovacích údajů AI při přepínání režimu přístupu. Osobní klíč API Gemini a přístupový kód Sonarpad AI se nyní ukládají nezávisle: přepnutí mezi osobním klíčem API a Sonarpad AI již nesmaže neaktivní přihlašovací údaj. Klíč Gemini se také uloží při opuštění jeho pole.

5. Pro „Znovu analyzovat segment“ a „Použít segment a znovu exportovat MP3“ bylo přidáno skutečné tlačítko Zrušit. Ukazatel průběhu zůstává aktivní a Zrušit nyní ukončí přípravu segmentu ve FFmpegu, AI worker, kontrolu TTS i závěrečný export, jakmile lze právě probíhající krok bezpečně zastavit. Použití znovu analyzovaného segmentu je nyní transakční: stávající projekt a výstupní médium se nahradí až po úspěšném opětovném exportu; při zrušení zůstanou předchozí soubory beze změny a návrh znovu analyzovaného segmentu zůstane k dispozici pro další pokus.

6. Opravena opětovná analýza segmentů pro popisy mimo první chunk Gemini. Izolovaný chunk se nyní odesílá stávajícímu workeru s místní časovou osou začínající od 0 a poté se znovu mapuje na absolutní pozici v projektu, čímž se zabrání chybě `Invalid prepared chunk timeline at chunk 1` bez změny běžné pipeline audiopopisu.

7. Opětovná analýza segmentů v uložených projektech nyní používá stejné bezpečnostní kontroly jako úplné vytvoření audiopopisu. Vybraný úsek prochází stejnou extrakcí mono zvuku 16 kHz a analýzou dialogů/ticha pomocí Pyannote, stejnými časovými pravidly a záložními postupy Gemini, stejnou skutečnou syntézou TTS hlasem projektu a stejným konečným plánovačem bezpečného umístění a prodloužených pauz. Při použití segmentu Sonarpad zachová nové ověřené absolutní časy a aktualizuje ochranu Pyannote pro daný úsek; předchozí speciální obcházení náhledu pro příliš dlouhý znovu analyzovaný text bylo odstraněno.

8. Opravena strukturální náhrada segmentu po úplné nové analýze. Sonarpad již nevyžaduje, aby nově ověřený segment obsahoval přesně stejný počet popisů jako uložený projekt: staré popisy daného chunku jsou nahrazeny celým bezpečným výsledkem úplné pipeline, takže segment s 10 popisy může správně přejít na 9 (nebo 11). Editor projektu okamžitě zobrazí nový počet a nové časy pro náhled, zatímco uložený projekt a mediální soubor zůstanou beze změny až do úspěšného dokončení použití segmentu a opětovného exportu MP3.

9. Přepracována opětovná analýza segmentu tak, aby se vybraný fyzický úsek zpracoval jako skutečný samostatný minifilm. Sonarpad nyní tento minifilm předává přímo přesně stejné funkci `create_audio_description`, která se používá při běžném úplném vytvoření, takže obraz a zvuková stopa sdílejí stejnou místní časovou osu a běžné kontroly Pyannote, Gemini, skutečného TTS, plánování a bezpečnosti proběhnou bez samostatné implementace opětovné analýzy. Teprve po dokončení této běžné pipeline se výsledné místní časy posunou zpět na původní pozici ve filmu.

Úpravy textu
1. Vylepšena funkce „Spojit zalomené řádky“ v nabídce Úpravy > Text. Bez výběru zpracuje celý dokument, s výběrem pouze označené řádky. Nyní rekonstruuje celé odstavce spojením po sobě jdoucích řádků i tehdy, když věta končí interpunkcí; prázdné řádky zůstávají oddělovači odstavců a číslované i odrážkové seznamy jsou zachovány.

Verze 0.9.6 – 2026-09-09

1. Opraveno zamrzání některých hlasů SAPI4 při zapnutém sledování kurzoru. Bridge nyní čeká na skutečné dokončení syntézy a sledování kurzoru zůstává zapnuté.

2. Opravena synchronizace mikrofonu a systémového zvuku a praskání způsobené zaokrouhlováním časových značek při nahrávání podcastů. Opravy platí i při samostatném ukládání zdrojů.

3. Syntéza audiopopisů pomocí SAPI5 nyní běží v oddělených procesech. Při selhání se opakuje s menším počtem souběžných procesů, zachová dokončené popisy a uloží funkční limit pro daný hlas.

Verze 0.9.5 – 2026-09-07

Audiopopis s AI
1. Zvýšena odolnost při dočasné odpovědi Gemini `MALFORMED_RESPONSE` bez použitelného obsahu. Sonarpad nyní zopakuje přesně stejný segment až třikrát s krátkou prodlevou mezi pokusy, místo aby okamžitě ukončil celý audiopopis. Běžné odpovědi Gemini, bezpečnostní blokace, chyby limitu tokenů i stávající chování kontrolních bodů zůstávají beze změny.

2. Opraven další vzácný případ chyby `invalid Gemini chunk timeline` u některých videí, jejichž chunky připravené pomocí FFmpeg vykazují malou kumulovanou odchylku délky. Sonarpad ponechává dosavadní postup beze změny, pokud je časová osa platná, a opatrné vyrovnání použije pouze tehdy, když by běžná časová osa selhala a celková odchylka je malá; velké nebo podezřelé rozdíly nadále skončí chybou.

3. Přidána volitelná služba Sonarpad AI pro audiopopis s AI. Uživatelé mohou nadále používat vlastní klíč Gemini API nebo zvolit službu Sonarpad a zadat osobní kód Sonarpad. Kód je chráněn pomocí Windows DPAPI. V režimu služby se připravené video segmenty nahrávají přímo z počítače uživatele do Googlu přes dočasnou adresu Google pro nahrávání; sonarpad.com nepřijímá ani neukládá data videa. Backend pouze autorizuje přístup, vynucuje model Gemini/úroveň Standard a limity použití, eviduje spotřebu a vrací odpověď Gemini.

4. Vylepšena úprava uložených projektů audiopopisu. Nyní lze upravit více popisů za sebou bez nutnosti každou změnu ihned použít. Změny zůstávají v paměti; po stisknutí „Použít text“ Sonarpad ověří každý upravený popis vůči jeho uloženému časovému rozsahu a poté uloží všechny platné změny společně. Pokud se některý popis do rozsahu nevejde, fokus se vrátí na tento popis a ostatní čekající změny se neztratí.

5. Přidáno „Upravit hlas“ bezprostředně před „Vytvořit audiopopis“. Nové okno umožňuje zvolit systém syntézy, hlas, rychlost a hlasitost a obsahuje „Vyzkoušet hlas“. Po stisknutí OK se fokus vrátí přesně na „Upravit hlas“ a zvolené hodnoty se použijí pro vytvářený audiopopis. Pokud okno nepoužijete, syntéza zůstane přesně jako dříve.


6. Bylo rozšířeno nastavení hlasu v uložených projektech audiopopisu. V úpravě projektu jsou nyní kromě enginu a hlasu k dispozici také rychlost, hlasitost a „Vyzkoušet hlas“. Při použití hlasových parametrů Sonarpad znovu ověří všechny existující popisy s novými parametry syntézy a MP3 znovu vytvoří pouze tehdy, pokud se všechny popisy stále vejdou do uložených časových intervalů. Intervaly bez dialogu, časy Visual Evidence, popisy Gemini a časová analýza projektu se znovu použijí bez opětovného spuštění analýzy AI.

Verze 0.9.4 – 2026-09-04

Audiopopis s AI
1. Opraven problém s videi Matroska/WebM, která obsahují neobvykle vysoké interní počáteční časové razítko a mohla zastavit vytváření audiopopisu chybou „invalid Gemini chunk timeline“. Sonarpad nyní normalizuje délku zdrojového souboru a segmentů Gemini pouze při zjištění tohoto neobvyklého vzoru časových razítek, takže běžná videa a dosavadní funkční chování audiopopisu zůstávají beze změny.
2. Vylepšen ducking audiopopisu pro přirozenější výsledný mix. Původní zvuk se nyní plynule ztiší ještě před začátkem hlasu a po skončení popisu se postupně vrací na běžnou úroveň; u popisů, které následují těsně po sobě, zůstává úroveň pozadí stabilní a nedochází k opakovanému zeslabování a zesilování.
3. Opraven problém, který mohl způsobit selhání závěrečného exportu audiopopisu u některých souborů AVI a dalších zdrojů se zvukem 5.1/vícekanálovým zvukem, pokud dekódování skončilo neúplným prokládaným zvukovým rámcem. Sonarpad nyní bezpečně doplní pouze tento poslední částečný rámec nezbytným počtem tichých vzorků; již úplné rámce a běžné mono, stereo i vícekanálové soubory zůstávají beze změny.


Verze 0.9.3 – 2026-09-03

Hlasy SAPI5
1. Opraven problém, kvůli kterému některé místní hlasy SAPI5 nemusely mluvit při zapnutém posouvání kurzoru, při čtení více hlasy nebo při vytváření audioknih/MP3. Sonarpad nyní používá cestu syntézy SAPI5 do souboru, která ve Windows spolehlivě funguje a zároveň umožňuje syntézu zrušit; běžné přímé přehrávání zůstává beze změny.
2. Opravena poloha kurzoru při čtení dialogů více hlasy. Hlasové značky, které Sonarpad automaticky vkládá pro dialogy, se nyní považují pouze za metadata přehrávání a ne za znaky v editoru, takže po F4 nebo F6 kurzor nepřeskočí před skutečný text. Čtení jedním hlasem a explicitní značky <voice> v dokumentu zůstávají beze změny.

Audiopopis s AI
1. Bezprostředně za pole klíče API Gemini bylo přidáno zaškrtávací políčko „Zobrazit klíč API“. Ve výchozím nastavení je vypnuté; po zapnutí dočasně zobrazí celý klíč, aby bylo možné ověřit, že byl vložen celý. Po opětovném otevření okna je klíč znovu skrytý.

Podcasty a Wikipedie
1. Po uložení nahrávky podcastu se Sonarpad nyní zeptá, zda má otevřít složku obsahující uložený soubor, stejně jako po uložení médií z YouTube/streamingu.
2. Při importu dalšího článku z Wikipedie do editoru, který už obsahuje text, se nový článek nyní přidá na konec místo vložení na začátek. Kurzor se umístí na začátek nově importovaného článku.


Verze 0.9.2 – 2026-09-02

Audiopopis s AI
1. Opraven problém, který mohl způsobit selhání audiopopisu s AI při závěrečném exportu do MP3 u videí s vícekanálovým zvukem, například 5.1. Sonarpad nyní automaticky převádí vícekanálový zvuk na stereo pouze tehdy, když je to nutné pro kódování MP3, aniž by měnil export mono nebo stereo zvuku.
2. Při spuštění audiopopisu s AI u videa s více zvukovými stopami se Sonarpad nyní před zpracováním zeptá, kterou stopu použít. Přístupný rozbalovací seznam lze měnit šipkami; OK spustí audiopopis s vybranou stopou, zatímco Zrušit zavře okno audiopopisu a vrátí fokus do editoru Sonarpadu.

YouTube a streamování
1. Opraven problém, kdy při spuštění audiopopisu s AI u videa na stránce 2 nebo další stránce playlistu či kanálu YouTube mohlo dojít k opětovnému otevření okna výběru YouTube a odebrání fokusu oknu audiopopisu. Sonarpad nyní výběr správně zavře bez návratu na předchozí stránky.
Verze 0.9.1 – 2026-09-01

Stahování z YouTube
• Opraven problém, kdy se okna průběhu stahování z YouTube/streamingu mohla po přepnutí do jiné aplikace pomocí Alt+Tab opakovaně vracet do popředí. Stahování nyní pokračuje na pozadí bez přebírání fokusu.
• Zlepšena přístupnost průběhu stahování. Po návratu do okna průběhu mohou čtečky obrazovky přečíst aktuální stav a procenta. U playlistů Sonarpad také oznamuje číslo aktuální položky, celkový počet položek a název.
• Opraveno falešné hlášení zamrznutí watchdogem během dlouhého stahování a převodu, když okno průběhu stále reagovalo.
• Do stahování playlist byla přidána rozbalovací nabídka Formát. V seznamu videí lze klávesou Tab přejít na volbu MP4, MP3, M4A, OPUS, OGG, WAV nebo FLAC před spuštěním hromadného stahování.
• Ukládání streamovaných médií bylo přepracováno. Formát a kvalita se nyní vybírají až při ukládání, nikoli v úvodním okně pro vyhledávání streamu. „Uložit médium“ otevře jeden dialog pro Formát a Kvalitu a stahování playlistů nabízí oba rozbalovací seznamy.

Audiopopis s AI
• Opraven problém, který mohl u některých videí MKV zabránit spuštění audiopopisu s AI. Sonarpad nyní spolehlivěji zpracovává videa s nepravidelnými nebo chybějícími časovými značkami.

Verze 0.9.0 – 2026-08-31

Audiopopis s AI — nová hlavní funkce
• Do Nástroje > Multimédia byla přidána funkce „Vytvořit audiopopis s AI“. Sonarpad analyzuje zvuk, vyhledá místa bez dialogů, vytvoří popisy pomocí Gemini a použije již dostupné hlasové moduly, aniž by mluvil přes dialogy.
• Byla zlepšena synchronizace mezi děním ve videu a popisy a časy vytvořené Gemini jsou automaticky kontrolovány.
• „Povolit rozšířené pauzy“ je ve výchozím nastavení vypnuto. Lze je zapnout u obsahu s mnoha dialogy nebo malým volným prostorem, aby bylo možné vložit delší popisy.
• Sonarpad se může pokusit rozpoznat postavy a používat jejich jména. Katalogy postav lze zachovat mezi epizodami seriálu pro lepší kontinuitu.
• Projekt lze uložit, později upravit popisy a znovu exportovat bez nutnosti vše znovu generovat pomocí Gemini.
• Pokud je proces přerušen, Sonarpad zachová průběh a umožní v audiopopisu pokračovat. Při vyčerpání kvóty Gemini lze čekat, změnit model nebo práci ukončit bez ztráty již dokončené části.
• V okně lze zvolit jazyk, úroveň podrobnosti, model Gemini, hlasový modul a hlas a použité nastavení se pamatuje.
• Modul je dostupný ve všech 17 jazycích Sonarpadu. Během generování rozhraní zobrazuje jen průběh, aktuální stav a Zrušit; po dokončení lze MP3 otevřít přímo v interním přehrávači.

E-knihy a dokumenty
• Přidán import Kindle bez DRM ve formátech MOBI, AZW a AZW3; text a kapitoly jsou dostupné v editoru a indexu.
• Přidána podpora DAISY 2.02 a DAISY 3. Audioknihy DAISY používají interní přehrávač Sonarpadu a respektují navigaci a hranice kapitol.
• Kindle a DAISY se importují bez přepsání původního souboru; Kindle chráněné DRM jsou výslovně odmítnuty.
• Opraveno „Uložit jako“ pro EPUB: při volbě TXT nebo jiného formátu se nyní použije vybraná přípona a původní EPUB zůstane spojen s otevřeným dokumentem.

RSS a články
• Přidán vícenásobný výběr článků RSS, takže lze několik článků odstranit v jediné operaci.
• RSS nyní podporuje skutečné složky zachované při importu i exportu OPML, včetně prázdných složek.
• Kanály lze v aktuální složce řadit pomocí Přesunout nahoru, Přesunout dolů, Přesunout na začátek, Přesunout na konec a Přesunout na pozici.

Přístupnost, návody a rozhraní
• Návody Sonarpadu byly přeorganizovány a doplněny o obsah a úplný návod k audiopopisu s AI.
• Opraven problém německého překladu, který mohl zabránit zobrazení dialogů Otevřít, Uložit jako a dalších dialogů pro výběr souborů.

Hlasy a jazyky
• Katalog hlasů Google TTS ke stažení byl rozšířen ze 104 na 156 balíčků a z 53 na 81 jazykových variant.
• Byly přidány nové balíčky Google TTS a lokalizované názvy dalších jazyků v celém rozhraní.

Verze 0.8.4 – 2026-07-24

Úpravy dokumentů EPUB
• Sonarpad nyní dokumenty EPUB nejen otevírá, ale umožňuje je také upravovat a znovu ukládat ve formátu EPUB se zachováním původního formátování, obsahu, poznámek pod čarou, obrázků, stylů, metadat a interních odkazů.
• Formát EPUB je v dialogu „Uložit jako“ dostupný u dokumentů otevřených ze souboru EPUB. Při ukládání se aktualizuje pouze změněný text a struktura knihy zůstává zachována.

Spolehlivost audioknih
• Opraven občasný problém, kdy byla po pěti neúspěšných pokusech Google TTS jednotka syntézy tiše zahozena a ve výsledné audioknize mohla chybět část textu.
• Jednotky Google se nyní opakují až do úspěchu nebo do zrušení uživatelem. Spouštění procesů je časově rozloženo, aby se omezily dočasné konflikty s Chromem a soubory; Sonarpad také tvorbu ukončí místo uložení audioknihy s chybějícím segmentem.
• Audioknihy Edge nyní při dočasných chybách sítě, WebSocketu, vypršení časového limitu, omezení služby nebo neplatném zvuku opakují pokus bez pevného limitu, dokud se nezdaří nebo je uživatel nezruší, včetně smíšených hlasů a dělení podle času. SAPI4 a SAPI5 si zachovávají adaptivní omezený počet pokusů; pokud se segment stále nedaří vytvořit, Sonarpad proces zastaví a neuloží neúplnou audioknihu.

Navigace v digitálních knihovnách
• Výsledky LibriVox, Internet Archive a Project Gutenberg nyní používají stránkovou navigaci jako YouTube: „Přejít na předchozí výsledky“ je na začátku seznamu a „Přejít na další výsledky“ na jeho konci.
• Bylo opraveno předávání fokusu v LibriVox: při otevření knihy nebo kapitoly se fokus NVDA již nepřesune do hlavního editoru před otevřením dalšího seznamu nebo přehrávače.
• Při vyhledávání a načítání knih LibriVox byla přidána ochrana fokusu: lokalizované okno načítání zůstává po celou dobu požadavku v popředí, takže fokus NVDA nepřeskočí do příkazového řádku, Windows Terminalu ani jiné aplikace.

Stahování playlistů YouTube
• Do playlistů YouTube byl přidán přístupný příkaz pro vícenásobný výběr, který umožňuje zvolit videa ke stažení, aniž by se změnil stávající příkaz „Uložit médium“ pro právě přehrávanou položku.
• Vybrané položky se stahují postupně ve formátu a kvalitě zvolených při otevření playlistu, dostávají číslované názvy zachovávající původní pořadí a ukládají se do samostatné složky v nastavené složce Média.
• Okno obsahuje příkazy „Vybrat vše“ a „Zrušit výběr“, oznamuje počet vybraných položek, umožňuje zrušení se zachováním již dokončených souborů a jasně vypíše položky, které se nepodařilo stáhnout.
• Položky playlistu jsou nyní nativní zaškrtávací políčka: čtečky obrazovky automaticky oznamují název, typ ovládacího prvku a stav zaškrtnutí, bez přidávání slov do viditelného názvu a bez vynucených hlasových hlášení.

Verze 0.8.3 – 2026-07-23

Tmavý režim
• Přidán tmavý režim, který lze zapnout v nabídce Zobrazení a který se ukládá do uživatelských nastavení.
• Tmavý motiv se použije v editoru, nabídkách, vedlejších oknech a hlavních ovládacích prvcích; barvy textu se přizpůsobí tak, aby zůstala zachována čitelnost a přístupnost.

Německý jazyk
• Přidána němčina jako úplný jazyk uživatelského rozhraní, který lze vybrat v Možnostech.
• Zprávy a RSS, kontrola pravopisu, kalendář a všechny citáty, informace o darech, nápověda i přehled změn jsou kompletně dostupné v němčině.

Brazilská portugalština a Zprávy Google
• Byla přidána brazilská portugalština jako úplný jazyk rozhraní, oddělený od portugalštiny používané v Portugalsku a volitelný v Možnostech.
• Celé rozhraní, kalendář a všechny citáty, kontrola pravopisu, dary, příručka a seznam změn jsou dostupné v brazilské portugalštině.
• Zprávy Google nyní podporují brazilské místní nastavení, brazilské kategorie a samostatné výchozí brazilské zdroje RSS.
• Pokud je kanál poskytuje, související zdroje stejné zprávy se zobrazí jako přístupné podřízené položky ve stromu.

LibriVox
• Vyhledávání v LibriVoxu bylo optimalizováno, aby nedocházelo k nadměrnému počtu požadavků na službu a k zamrzání rozhraní. Bylo odstraněno rozsáhlé procházení katalogu, snížen počet pokusů a zavedeny kratší časové limity.

Syntéza řeči
• Posloupnosti tří nebo více teček se nyní před čtením normalizují, takže některé hlasy již nevyslovují „tečka tečka“ ani nevytvářejí úseky tvořené pouze interpunkcí.

Související články Google News
• U každé zprávy se nyní, pokud jsou k dispozici, zobrazují související články, tedy další články pojednávající o stejné události. Chcete-li si je přečíst, stačí rozbalit hlavní článek, když Sonarpad oznámí, že jsou k dispozici související články. Pokud tuto část nechcete rozbalit, stačí stisknout Enter na hlavním článku a přečíst si zprávu jako dosud.
• Související články nyní používají stejný systém přečteno/nepřečteno jako hlavní články, včetně přístupných oznámení, data a času, ukládání stavu a jeho zachování po aktualizaci zdrojů nebo restartování Sonarpadu.

Oznámení v částech audioknih
• Do možností zvuku byl přidán rozbalovací seznam „Oznámení na začátku každé části“. U audioknih rozdělených do více souborů může každá část začínat bez oznámení, názvem knihy, názvem a číslem části, názvem souboru nebo názvem souboru a číslem části.

Verze 0.8.2 – 2026-07-17

Digitální knihovny a audioknihy
• Přidán Project Gutenberg s vyhledáváním podle názvu nebo autora a s volbou jazyka.
• Knihy EPUB z Project Gutenberg se stahují do složky Dokumenty\Sonarpad\Documents; po dokončení se Sonarpad zeptá, zda má knihu ihned otevřít v editoru.
• Přidán Internet Archive pro vyhledávání a poslech zvukových sbírek, včetně historického rozhlasu, projevů a živé hudby.
• Přidán LibriVox pro vyhledávání audioknih podle názvu nebo autora a přímé přehrávání kapitol stejným přehrávačem, jaký používají podcasty.
• Všechny tři nové funkce jsou dostupné v nabídce Nástroje a při zapnutém seskupování nabídek také v části Čtení.

Dlouhé zvukové přepisy
• Opraven přepis dlouhých zvukových souborů: zvuk se nyní automaticky rozdělí na patnáctiminutové části, postupně se přepíše a poté znovu spojí, čímž se předchází chybám u dlouhých nahrávek.

YouTube
• Nejužitečnější akce, které byly dříve dostupné jen po otevření videa na YouTube a vstupu do nabídky Přehrávání, jsou nyní dostupné také přímo v kontextové nabídce stejného videa, například „Přepsat aktuální audio“, „Vytvořit audiopopis pomocí AI“ a „Uložit médium“, pro snazší používání.
• Přidána položka „Kopírovat odkaz“, dostupná také pomocí Ctrl+C, která zkopíruje do schránky URL vybraného videa, playlistu nebo kanálu YouTube.

Verze 0.8.1 – 2026-07-16

Google TTS
• Opraveno spouštění Google TTS v systémech Windows, kde připojení přijatá interním serverem prohlížeče dědila neblokující režim socketu, což způsobovalo chybu 10035 a znemožňovalo přehrávání stažených hlasů.
• Sonarpad nyní před náhledem hlasu nebo čtením pomocí F5 čeká na úplné načtení modulu WASM v Chromu nebo Edgi, čímž se předchází chybě „Chrome WASM TTS engine was not loaded“.
• Skrytý prohlížeč vypíná překlad stránek a zpřístupnění vykreslovacího procesu, aby neoznamoval možnosti jako „Přeložit stránku“ a nenarušoval příkazy čtení.
• Panel „Hlasy v editoru“ nyní při výběru enginu Google zobrazuje tlačítko „Spravovat hlasy Google...“ a po zavření správce ihned obnoví seznam nainstalovaných hlasů.
• Upozornění na závislosti zobrazovaná při odebírání hlasových balíčků Google jsou nyní přeložena do všech jazyků rozhraní.

Průběh aktualizace
• Po automatické aktualizaci se okno dokončení se seznamem změn otevře až po počátečním obnovení fokusu a zůstane v popředí, místo aby se zobrazilo teprve po stisknutí klávesy Tab.

Dokumenty PDF
• Opraveny soubory PDF, jejichž vložený text obsahoval znaky NUL a při načtení do editoru se u prvního z nich ořízl.
• Pokud pdf-extract vrátí vložené znaky NUL, Sonarpad zkusí extrakci znovu pomocí PDFium; zbývající znaky NUL se před předáním textu ovládacím prvkům Windows odstraní, takže zbytek dokumentu zůstane zachován.

Přístupnost nabídek
• Výpočet mnemotechnických kláves za běhu byl odstraněn: přístupové klávesy jsou nyní výslovně zapsány ve všech 15 překladech rozhraní a při každém spuštění zůstávají stejné.
• Byly zkontrolovány všechny stabilní položky hlavních nabídek a podnabídek, včetně Přehrávání, písem, Uložit obrázek a Zobrazit rejstřík EPUB; chybějící nebo duplicitní mnemotechnické klávesy mezi položkami stejné úrovně byly opraveny přímo v překladech.
• Automatické testy nyní překlady pouze ověřují a selžou, pokud mnemotechnická klávesa chybí, je neplatná nebo duplicitní; za běhu již popisky nemění.
• U mimořádně rozsáhlých nabídek, kde překlad neposkytuje dostatek různých znaků, se zobrazí výslovná číselná přístupová klávesa ve standardním tvaru Windows „(&1)“.

Verze 0.8.0 – 2026-07-15

Online slovník
• Do online slovníku Wiktionary byla přidána němčina.
• Německé definice a synonyma jsou nyní správně rozpoznávána podle struktury německého Wiktionary.

Spolehlivost audioknih SAPI5
• Vytváření audioknih SAPI5 nadále používá až 12 paralelních pracovníků, pokud vybraný hlas vytváří spolehlivý výstup.
• Každá část se kontroluje podle velikosti souboru, odhadované délky a opatrného porovnání s přiřazeným textem.
• Chybějící nebo podezřelé části se automaticky vytvoří znovu s postupně nižší souběžností: 12, 8, 6, 4, 2 a nakonec 1 pracovník. Opakují se pouze problematické části.
• Spolehlivý limit se ukládá samostatně pro každý hlas SAPI5, aniž by zpomaloval hlasy, které správně fungují s 12 pracovníky.
• Závěrečná kontrola zabrání tichému přijetí MP3, které je výrazně kratší než vytvořené části.
• Podrobnosti se zapisují do `sapi5_audiobook_diagnostic.log`.
• Každá jednotka syntézy SAPI5 nyní běží v samostatném skrytém procesu Sonarpad. Pokud hlas třetí strany selže, ukončí se pouze tento worker a hlavní aplikace zůstane spuštěná.
• Během stejného vytváření audioknihy se nedokončené části okamžitě zopakují s následující nižší úrovní souběhu; již ověřené části se zachovají.
• Obnova při příštím spuštění zůstává doplňkovou ochranou pouze pro případ přerušení hlavní aplikace nebo počítače.

Procesy audioknih SAPI4
• Počet procesů SAPI4 zvolený uživatelem je nyní respektován až do technického maxima 64; předchozí skrytý limit 16 byl odstraněn.
• Skutečný počet se sníží pouze tehdy, když audiokniha obsahuje méně pracovních jednotek, než bylo požadováno.
• Pokud jeden nebo více procesů mostu SAPI4 selže, dokončené části se zachovají a automaticky se zopakují pouze neúspěšné jednotky s postupně nižší souběžností.
• Sonarpad nyní kontroluje návratový kód mostu SAPI4 a odmítne prázdné nebo neplatné zvukové části.

Nastavení proxy
• Do nastavení sítě bylo přidáno samostatné pole pro port proxy.
• Port lze zadat nezávisle na adrese proxy, je ověřen v rozsahu 1 až 65535 a správně nahradí port, který již může být součástí adresy.

Vyhledávání rádia podle jazyka a země
• Filtry Jazyk a Země se nyní aktualizují všemi položkami dostupnými v katalogu Radio Browser a nejsou již omezeny na pevný seznam.
• Názvy jazyků jsou nyní rozpoznány i tehdy, když je Radio Browser poskytne v jiném písmu, v rodném názvu, jako zkratku nebo jako kombinaci více jazyků, a zobrazí se přeložené do aktuálního jazyka rozhraní. Hodnoty, které nepředstavují skutečné jazyky, například čísla, hudební žánry, země nebo obecné popisy, jsou odfiltrovány.
• Katalog se aktualizuje na pozadí a při nedostupnosti Radio Browseru zůstává k dispozici záložní seznam.
• Duplicitní jazykové položky Radio Browseru, které jsou po překladu totožné, se nyní slučují do jediné položky rozbalovacího seznamu, aby čtečky obrazovky nezůstávaly při pohybu potichu.

Hlavní vylepšení: synchronizace čtení a kurzoru
• Synchronizace hlasového čtení a pohybu kurzoru byla výrazně vylepšena pro všechny podporované hlasové moduly.
• Je-li zapnuta volba „Posouvat kurzor během čtení“, používá Sonarpad společný systém postupu pro Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 a OneCore.
• Kurzor nyní přesněji sleduje skutečně vyslovovaný text a používá jednotnější dělení vět a jejich částí.
• Výrazně se omezilo předbíhání, zpoždění, nepravidelné skoky a rozdíly mezi jednotlivými hlasovými moduly.
• Správná pozice se lépe zachovává po pozastavení, obnovení, hledání v dokumentu nebo změně hlasového modulu.

Samostatné soubory při nahrávání podcastu
• Přidána volba „Uložit mikrofon a systémový zvuk nebo zvuk aplikací do samostatných souborů“.
• Při současném nahrávání mikrofonu a dalšího zdroje může Sonarpad vytvořit jeden soubor pouze s mikrofonem a druhý se systémovým zvukem, jednou aplikací nebo vybranými aplikacemi.
• Oddělené nahrávání je dostupné pro MP3 i WAV.
• Pokud volba není zapnuta, nadále se vytváří jeden smíchaný soubor.
• Samostatné soubory usnadňují úpravu hlasitosti, odstranění šumu a následný střih podcastů, rozhovorů a návodů.

Plánované nahrávání rádia
• Nahrávání rádia lze nyní naplánovat předem.
• Pro každé nahrávání lze vybrat stanici, den, hodinu a minutu zahájení a délku.
• K dispozici je vlastní délka od 1 do 1 440 minut.
• Nahrávání lze spustit jednou, denně nebo týdně.
• Okno přehledněji zobrazuje probíhající a plánovaná nahrávání, plánované datum a čas, délku a zbývající čas do spuštění.
• Pomocí Plánovače úloh Windows lze nahrávání spustit automaticky, i když Sonarpad není otevřen.

Kalendář
• Přidán úplný kalendář přístupný z klávesnice.
• Umožňuje procházet předchozí a následující dny, rychle se vrátit k dnešku a zjistit svátky a významné dny.
• Přidán svátek a citát dne, které lze přečíst, vyslechnout nebo zkopírovat.
• Připomínky lze vytvářet, upravovat, mazat, odkládat a označovat jako dokončené.
• Upozornění lze zobrazit přesně v určený čas nebo s předstihem a mohou fungovat prostřednictvím plánování Windows i při zavřeném Sonarpadu.

Počasí
• Přidána sekce předpovědi počasí.
• Lze vyhledat město a rychle znovu otevřít nedávno zobrazená místa.
• Dostupné jsou aktuální podmínky, teplota, minimum a maximum, vlhkost, pravděpodobnost srážek a předpověď na další dny.
• Lze zvolit stupně Celsia, Fahrenheita nebo automatický výběr.

Filmy v kinech
• Přidána sekce s filmy právě uváděnými v kinech a připravovanými premiérami.
• K dispozici je hledání podle názvu, popis děje, datum uvedení a přehrávání upoutávky.

Google TTS
• Přidán Google TTS pro čtení dokumentů a vytváření audioknih.
• Přidán správce hlasů pro jejich zobrazení, filtrování podle jazyka, stažení a odstranění nepotřebných hlasů.
• Lze nastavit rychlost, hlasitost a výšku hlasu.
• Výška hlasů Google Natural se nastavuje přímo v modulu, což přináší přirozenější a stabilnější výsledek.
• Zlepšena odezva a spolehlivost Google TTS a časové limity se přizpůsobují rychlosti hlasu.
• Zkráceno zbytečné čekání a vylepšeno zpracování chyb a přerušení.

Obsah dokumentů EPUB
• Sonarpad nyní rozpozná obsah vložený v knihách EPUB.
• Jeho přítomnost je oznámena a lze jej otevřít z nabídky Zobrazení.
• Kapitoly a podkapitoly jsou zobrazeny hierarchicky.
• Stisknutím Enter se okamžitě přejde na vybrané místo v knize.

Zprávy a zdroje RSS
• Sekce Zprávy byla rozšířena o nové možnosti vyhledávání a organizace.
• Přidán výběr jazyka zpráv.
• Lze vyhledávat ve zdrojích RSS a číst zprávy z vlastního města.
• Komunitní zdroje lze procházet, přidávat do vlastní sbírky a odesílat komunitě Sonarpad.

Nahrávání podcastu
• Lze nahrávat pouze mikrofon, celý systémový zvuk, jednu aplikaci, více vybraných aplikací nebo mikrofon a aplikace současně.
• Lze vybrat mikrofon a zdroj zvuku, samostatně nastavit hlasitost a sledovat úrovně v reálném čase.
• Přidáno pozastavení a obnovení, výstup MP3 nebo WAV, volba datového toku MP3 a cílové složky.
• Během nahrávání lze zabránit uspání počítače.

Rádio
• Sekce Rádio byla výrazně přepracována.
• Stanice lze hledat podle názvu nebo volného textu, jazyka, země, města, hudebního žánru nebo kategorie.
• Zlepšena správa oblíbených položek a přidáno rychlé vymazání všech filtrů.
• Stanice lze odesílat komunitě Sonarpad.
• Přidáno živé nahrávání, režim „Nahrávat a přehrávat“, seznam nahrávek a jejich správa a mazání.
• Nahrávky rádia jsou ukládány do vlastní složky v hlavním adresáři nahrávek.

Přehrávání médií
• Výrazně zlepšena stabilita přehrávače médií.
• Opraven problém, který mohl zablokovat mpv, a zlepšena komunikace s přehrávačem.
• Vylepšeno otevírání různých typů multimediálních souborů.
• Sonarpad si nyní pamatuje použitou hlasitost.
• Zlepšena správa streamů a nahrávek.
• Opraveno otevírání souborů z Windows pomocí dvojitého kliknutí nebo „Otevřít v programu“.

Dokumenty PDF
• Přidáno rozpoznávání formulářových polí v PDF.
• Sonarpad umí najít vyplnitelná pole, zpřístupnit je v textové podobě, umožnit úpravu a uložit údaje do PDF.
• Opraven výpočet pozice kurzoru při čtení, zejména u vícebajtových znaků a složitých struktur.

Přístupnost a klávesnice
• Zlepšeno fungování běžných editačních příkazů v celém programu.
• Kopírování, vyjmutí, vložení, výběr všeho, zpět a znovu se správně odesílají do prvku s fokusem, včetně vedlejších oken a dialogů.
• Opraven problém s aktualizací braillských řádků.
• Zlepšena práce s fokusem a opraven výběr jazyka ve Wikipedii.
• Přidána možnost seskupovat funkce nabídky Nástroje podle kategorií.
• Přidány nastavitelné akce pro rychlé otevření Kalendáře, Počasí a Filmů v kinech.

Audioknihy
• Zlepšeno vytváření audioknih při otevřených dialozích nebo modálních oknech.
• Sledování průběhu je robustnější a ignoruje zastaralé zvukové aktualizace.
• Google TTS lze použít také k vytváření audioknih s nastavením rychlosti, hlasitosti a výšky.

Umělá inteligence
• Výchozí model Gemini byl aktualizován na `gemini-3.5-flash`.

Obecné opravy
• Opraveno několik zamrznutí při přehrávání pomocí mpv.
• Opraveno otevírání některých zvukových a obrazových souborů.
• Zlepšena správa příkazů odesílaných přehrávači.
• Opraveno obnovení kurzoru během čtení.
• Zlepšena stabilita vytváření audioknih.
• Zlepšena celková správa médií, RSS, rádia a EPUB.

Verze 0.7.1 – 2026-05-13

Novinky a vylepšení
• Vytvořen oficiální web sonarpad.com, nové referenční místo pro sledování nejnovějších novinek, stažení nejnovější verze programu, čtení komentářů návštěvníků a v budoucnu také poslech všech podcastů Sonarpadu. Do nabídky Nápověda byla také přidána položka „Navštívit sonarpad.com“, která umožňuje rychle otevřít oficiální web.
• Opraven problém, kdy soubory s diakritikou nebo speciálními znaky způsobovaly chybu při spuštění hlasového přepisu.
• Od nynějška budou v nabídce Zobrazit položky jako Automatické zalamování řádků a Zobrazit video během přehrávání vždy zobrazovat správný stav, zapnuto nebo vypnuto.
• Vylepšeno vyhledávání na YouTube, s možností vrátit se klávesou Esc na předchozí stránku nebo obrazovku.
• Přidána předběžná kontrola, zda lze video přehrát. Vylepšeno bylo také přehrávání: Sonarpad nyní dokáže přehrát i videa nebo playlisty označené jako mix, které dříve přehrát nešlo.
• Vylepšena správa automatických záložek. Dříve, pokud byla možnost Automatické záložky zapnutá a poté vypnutá, tyto záložky zůstávaly; nyní je program správně ignoruje, dokud není možnost znovu zapnuta. Navíc se při dosažení konce mediálního souboru záložka automaticky odstraní.
• Vylepšena správa značek při zapnutých dialozích. Sonarpad nyní správně zvládá obě funkce a umožňuje vkládat značky i tehdy, když je volba dialogů zapnutá.
• Vylepšena nastavení hlasu jasným oddělením jednotlivých enginů, takže nastavení je přesnější. Hlasové profily nyní správně ukládají nastavení pro každý engine zvlášť: Edge, Sapi5 a Sapi4.
• Přidána značka pro vkládání pauz, přímo z možností nebo z panelu hlasů stisknutím Tab z editoru. Dostupné volby jsou: 250 ms, 500 ms, 1 sekunda, 2 sekundy nebo vlastní délka.
• Opraveno chování při přehrávání videa z YouTube a spuštění přepisu. Nyní se při návratu pomocí Alt+Tab fokus správně nastaví na tlačítko Zrušit probíhajícího přepisu.
• Přepisy se nyní po dokončení procesu ukládají automaticky.
• Vylepšen import z Wikipedie. Je možné zvolit, zda číst pouze jednu sekci a poté se z článku klávesou Esc vrátit k vyhledávání, nebo importovat celý článek. Lze také vybrat jazyk Wikipedie.
• Přidána sekce rádií z celého světa, kde lze vyhledávat rádio podle země, jazyka a žánru. Místní rádia lze také přidat do databáze Sonarpadu, aby je mohli poslouchat i ostatní uživatelé. Rádio lze také přidat do oblíbených.
• Přidána sekce tras pro výpočet cest podle zvoleného způsobu: pěšky, na kole, autem nebo na invalidním vozíku. Lze zvolit nejkratší nebo nejrychlejší trasu a také zobrazení projetých obcí. Po importu trasy lze vizuální mapu uložit z nabídky Soubor, Uložit obrázek.
• Do nabídky Soubor byla přidána položka Tisk. Sonarpad bude tisknout TXT soubory vlastním systémem a pro jiné soubory, jako DOCX, PDF a podobné, použije přidružený program, aby bylo co nejvíce zachováno původní rozvržení.
• Do Sonarpadu byla integrována služba pro překlad každého dokumentu, dostupná z kontextové nabídky editoru. Uživatel může bez zadání API klíče používat bezplatné služby DeepL a Google Translate; po zadání Gemini API klíče může překládat pomocí Gemini.
• V nabídce překladu si uživatel může zvolit cílový jazyk. Nabídka se automaticky přeuspořádá: pokud uživatel nejprve zvolí angličtinu, potom francouzštinu a potom italštinu, tyto tři volby se zobrazí nahoře v nabídce jazyků.
• Pokud uživatel zadá svůj Gemini API klíč, získá také přístup k funkci Shrnout text, která je také dostupná v kontextové nabídce a umožňuje shrnout jakýkoli článek.
• Do nabídky Přehrávání, viditelné při přehrávání mediálního souboru, byla přidána nabídka pro rozdělení aktuálního média. Funguje s MP3, MP4 a dalšími formáty, a umožňuje dělení podle počtu částí nebo podle délky každé části.

Verze 0.7.0 – 2026-04-25

Co je nového
• Přidána podpora přehrávače mpv pro streamované přehrávání. Videa z YouTube a podporovaných webů se nyní přehrávají okamžitě; pokud si je uživatel chce uložit, stáhnou se jako dříve. Při přepisu streamovaného obsahu se nejprve stáhne a poté přepíše. Přehrávač mpv se také používá pro otevírání lokálních videí a práci s titulky, což zajišťuje lepší kompatibilitu s mnoha formáty.
• Vylepšeno nahrávání podcastů systémového zvuku: nyní si můžete zvolit, zda chcete nahrávat veškerý systémový zvuk, jednu aplikaci nebo více aplikací současně. Tato volba je integrována do běžného nahrávání, takže mikrofon lze stále samostatně zapnout nebo vypnout.
• Přidán jazyk hindština. Rozhraní přeloženo, přidány RSS, seznam změn a příručka Sonarpad.
• Přidána možnost v kartě Editor, která při použití šipek nahoru a dolů vždy přesune kurzor na začátek řádku.
• Přidána možnost v nabídce "Převést audio" pro převod zvuku do formátu M4B.

Opravy
• Opravena klávesa `F10`, takže při čtení textu znovu přepíná na další oblíbený hlas.
• Když probíhá nahrávání podcastu, zavření jiného dokumentu už nezavře také aktivní nahrávání.
• V komentářích YouTube otevřených z „Přehrát streamované audio...“ nyní Sonarpad nejprve načte pouze prvních 50 komentářů nejvyšší úrovně, vždy včetně všech odpovědí k těmto komentářům, a na konci přidá položku pro načtení všech komentářů podle potřeby.
• Záložky se nyní zobrazují a zpracovávají podle své pozice jak v textových dokumentech, tak v multimediálních souborech, místo aby sledovaly pořadí vytvoření. Pokud už záložka na stejné pozici existuje, znovu se nepřidá.
• Do nabídky Záložky byla přidána volba, která po zapnutí umožňuje automatickou správu záložek. Při přehrávání místního nebo streamovaného souboru a jeho zavření Sonarpad automaticky nastaví záložku podle dosažené pozice a při opětovném otevření souboru bude pokračovat od tohoto místa. Totéž platí pro textové soubory: pokud otevřete text a přesunete kurzor, Sonarpad si po zavření tuto pozici zapamatuje; pokud spustíte čtení, uloží se poslední přečtená věta a čtení bude pokračovat přesně odtud.
• Do nabídky Zobrazit byla přidána položka pro zobrazení vykreslování videa u místních nebo streamovaných souborů. Video obsah se zobrazuje ve zvětšeném okně, ve kterém jsou všechny ovládací prvky skryté, kromě případů, kdy stisknete klávesu Alt nebo přesunete myš k horní části okna. Díky tomu by měli mít slabozrací uživatelé větší a lépe použitelný obsah.

Verze 0.6.9 – 2026-04-08

Opravy
• Vylepšeno používání funkce Najít v souborech: při otevření Procházet složku se zaměření nyní přesune přímo na seznam složek; otevření výsledku klávesou Enter už nenarušuje klávesové příkazy; stisknutí Esc vrátí zaměření na dříve vybraný výsledek; a při návratu pomocí Alt+Tab se zaměření přesune buď na pole hledání, nebo na seznam výsledků, pokud jsou výsledky otevřené.
• Klávesa F5 vždy spouštěla čtení od začátku. To bylo nyní opraveno a čtení začíná od aktuální pozice kurzoru, přičemž `Shift+F5` a `Ctrl+F5` zůstávají zachovány pro navigaci na předchozí a další větu.
• Po použití funkce Přejít na řádek mohlo stisknutí Esc přesunout zaměření mimo Sonarpad. Nyní správně vrací zaměření do editoru.
• Možnost „Zalamovat řádky“ se nyní použije okamžitě i na již otevřené dokumenty, místo aby se projevila až po znovuotevření souboru.

Verze 0.6.8 – 2026-04-07

Co je nového
• Přidána nová položka do nabídky Přehrávání, která umožňuje přepisovat jakýkoli audio nebo video soubor pomocí Whisper. V Možnostech je nyní dostupná nová sekce „AI a přepis“, kde lze vybrat model, zapnout volitelnou podporu CUDA pro grafické karty NVIDIA, zachovat původní jazyk a zapnout nebo vypnout časové značky.
• Přidána nová akce „Přepsat aktuální složku“ v nabídce Přehrávání, která zpracuje všechny podporované audio soubory ve složce právě otevřeného média do jednoho společného dokumentu, s vlastním ukazatelem průběhu, stavem aktuálního souboru a podporou zrušení. Lze ji také spustit pomocí Alt+Shift+C.
• Přidáno offline hlasové diktování, které využívá stejný postup jako přepis audia. Ve výchozím nastavení stiskněte Ctrl+Shift+Space pro spuštění diktování a stejnou zkratku znovu pro zastavení; tuto zkratku lze změnit v Možnostech. Od druhého použití je diktování rychlejší, protože modul zůstává připravený v paměti; toto přednačtení a opětovné použití se automaticky vypnou na počítačích s méně než 4 GB RAM.
• Přidána nová možnost Editoru, ve výchozím nastavení vypnutá, která umožňuje, aby Esc zavřelo okno editoru.
• Vyhledávání podcastů nyní ve výchozím nastavení používá iTunes + Spreaker, s filtrováním duplicit, pokud je stejný podcast nalezen na obou platformách.
• Vylepšeno procházení a vyhledávání Apple podcastů: vyhledávání podcastů, procházení kategorií a nejlepší podcasty podle kategorií nyní používají vybranou zemi adresáře podcastů. V Možnosti > RSS a podcasty můžete ponechat Automaticky pro použití systémové země, nebo ručně zvolit jinou zemi.
• Zvýšen limit výsledků pro kategorie Apple podcastů. Při prvním otevření se stále načte prvních 50 výsledků jako dříve; pokud zvolíte Načíst další výsledky, Sonarpad načte až 200 výsledků celkem (limit Apple) a umožní procházet další stránky při zachování plynulého ovládání.
• Sonarpad je nyní dostupný také na Macu s omezenou sadou funkcí. Odkaz na projekt: https://github.com/Ambro86/Sonarpad-Mac

Vylepšení
• Přidáno více než 50 volitelných zemí pro adresář podcastů, takže uživatelé mohou vybírat z mnohem širší nabídky národních katalogů.
• „Přehrát streamované audio...“ nyní umí také vyhledávat na YouTube podle libovolného textového dotazu nebo přijmout odkaz na YouTube kanál či playlist a zobrazit jeho výsledky.
• Vylepšeno zobrazení výsledků v „Přehrát streamované audio...“: položky YouTube nyní přehledněji zobrazují název, délku, kanál a počet zhlédnutí.
• „Přehrát streamované audio...“ nyní podporuje také komentáře YouTube: lze je otevřít z kontextové nabídky, číst odpovědi a rozbalovat vlákna komentářů pomocí klávesy Šipka vpravo.
• Přidány oblíbené položky YouTube pro kanály a playlisty v „Přehrát streamované audio...“: lze je přidat z výsledků přes kontextovou nabídku, otevřít přímo ze seznamu Oblíbené, který je dostupný klávesou Tab hned za polem URL/dotazu YouTube, a později je odstranit z téhož seznamu pomocí kontextové nabídky. Ve výsledcích hledání YouTube je kontextová nabídka dostupná pouze pro kanály a playlisty.
• „Přehrát streamované audio...“ nyní může vyžadovat přihlašovací údaje, když streamovací web vyžaduje přihlášení. Uživatelé je mohou zadat, uložit pro daný web a později spravovat uložené přihlašovací údaje v Možnosti > Audio.
• Vylepšena práce se zaměřením během „Přehrát streamované audio...“, takže okno průběhu zůstává stabilnější během stahování a převodu.
• Přidány dvě nové akce pro navigaci při čtení v nabídce Hlas a zvuk: Předchozí věta a Další věta, s nastavitelnými zkratkami pro skoky při čtení textu.
• Výchozí zkratka pro Spustit soubor interpretem je nyní Ctrl+Shift+F5, takže Shift+F5 lze ve výchozím nastavení použít pro akci Předchozí věta.
• Přidány hlasové profily v Možnosti > Hlas: profily lze přidávat, používat a mazat.
• Rozšířeny možnosti intervalu přeskočení médií v Možnosti > Audio o další hodnoty od 1 sekundy až do 2 hodin.
• Přidán ruský překlad díky Dmitriyovi.
• Přidána nová možnost v Možnosti > Audio pro výběr formátu pojmenování částí audioknihy: Název + číslo, Pouze číslo nebo Číslo + název.
• Přidány oblíbené články RSS: z kontextové nabídky článku lze položky přidat do zvláštního kanálu Oblíbené.
• RSS kanál Oblíbené lze smazat a při přidání nového článku do oblíbených se automaticky znovu vytvoří.
• Přidány klávesové zkratky RSS pro přesun kanálů nahoru/dolů: Ctrl+Shift+Šipka nahoru a Ctrl+Shift+Šipka dolů.
• Vylepšeno okno RSS o vestavěný náhled článku, takže text článku lze zkontrolovat přímo tam a rychle k němu přejít pomocí Tab ještě před otevřením celého článku v editoru.
• Přidána výslovná položka RSS „Načíst další zprávy“ na konci kanálů, pokud jsou dostupné další položky; stisknutí Enter načte další dávku a přesune zaměření na první nově načtený článek.
• Ve slovníku hlasů je nyní při přidávání nebo úpravě náhrady k dispozici zaškrtávací pole „Rozlišovat velikost písmen“, takže každá náhrada může buď respektovat, nebo ignorovat velikost písmen.

Opravy
• „Přehrát streamované audio...“ nyní respektuje limit mezipaměti podcastů již nastavený v Možnostech a stejný limit se nyní vztahuje také na přehrávání audiopopisů.
• Opraven import z Wikipedie, takže bloky citací přítomné na stránkách se nyní importují správně.
• Vylepšen parser webových stránek pro stránky WordPress, kde mohly být vynechány položky seznamu a některé nadpisy sekcí.
• „Přejít na řádek“ nyní předvyplní pole aktuálním číslem řádku.
• Opraven export OPML pro podcasty a RSS, takže exportované soubory jsou nyní přijímány iTunes.
• Přidány lokalizované potvrzovací zprávy pro správný import a export OPML RSS kanálů a podcastů.
• Opraven problém, kdy v „Přehrát streamované audio...“ zadání hledaného textu a výběr YouTube kanálu z výsledků mohl způsobit, že program vypadal jako zaseknutý, místo aby otevřel videa daného kanálu.
• Opraven problém, kdy se seznam otevřených dokumentů zobrazoval v nabídce Nápověda místo v nabídce Okno.
• Opraven okrajový problém streamování, kdy se přehrávání mohlo spustit, ale dialog „Stahování streamu“ zůstal otevřený, když stažený soubor již odpovídal cílovému formátu.
• Opraveno chování převodu MP3 streamů: pokud je stream již ve formátu MP3 a uživatel zvolí konkrétní bitrate MP3 (například 128 kbps), Sonarpad nyní znovu zakóduje na vybraný bitrate místo přeskočení převodu.
• Opraveny dokumenty přepisu médií, takže jejich zavření nyní vyžaduje potvrzení uložení a navrhovaný název souboru správně znovu používá název přepsaného mediálního souboru místo prvního řádku textu.
• Opravena zkratka Alt+Shift+L: nyní správně otevírá seznam kapitol během přehrávání.
• Opravena zkratka Alt+Shift+T: nyní správně spouští „Přepsat aktuální audio“ místo otevření nabídky Nástroje.
• Opraveno zastavení přehrávání v nabídce Přehrávání: stisknutí . se nyní chová jako Zastavit a zastaví pouze aktuální stopu místo toho, aby zároveň ukončilo přehrávač/epizodu.
• Opravena položka uložení v nabídce Přehrávání pro média otevřená z Nedávných souborů: pokud soubor pochází z místní cache Sonarpad, lokalizovaná akce uložení se nyní správně zobrazuje i tam.
• Když přepis začne ve chvíli, kdy se již přehrává audio, Sonarpad nyní toto audio automaticky pozastaví před zahájením přepisu.
• Opraven problém, kdy import článku z Wikipedie mohl uspět, aniž by se text článku zobrazil na obrazovce.
• Přidána podpora vložených kapitol podcastů z místních mediálních souborů (např. metadata kapitol MP3): když nejsou k dispozici kapitoly z feedu/URL, Sonarpad nyní načte kapitoly ze staženého souboru na pozadí, takže přehrávání začne okamžitě a data kapitol se použijí, jakmile budou připravena.
• Opraveno načítání kapitol u stažených epizod podcastů otevřených jako běžné místní mediální soubory: vložené kapitoly jsou nyní dostupné i zde, nejen když přehrávání začíná z okna Podcasty.
• Opravena finalizace MP3 audioknih pro SAPI4 a SAPI5: konečný výstup je nyní správně dokončen, aby se předešlo neúplným nebo křehkým souborům po dlouhých exportech.
• Přidán výslovný ukazatel průběhu finalizace pro všechny režimy vytváření audioknih: po fázi vytváření nyní Sonarpad oznamuje a zobrazuje zvláštní fázi finalizace s viditelným průběhem.
• Opraveno ladění hlasů dialogů: nastavení rychlosti/výšky/hlasitosti se nyní správně používá pro první i druhý hlas dialogu během syntézy.
• Vylepšena detekce kódování textu pro japonské soubory .txt: přidána bezpečná záložní volba Shift_JIS/CP932 pro případy zkomoleného textu, při zachování stávajícího chování pro UTF/diakritiku/čínštinu.
• Interní bezpečnostní refaktor: funkce byly tam, kde to bylo možné, převedeny na bezpečné implementace a počet řádků s nebezpečným kódem byl výrazně snížen.

Verze 0.6.7 – 2026-03-02

Vylepšení
• Program nyní dokáže hromadně zpracovat funkci Nahradit vše i u velkých souborů s velmi vysokým počtem nahrazení.
• Aktualizován polský překlad díky DJ Graco.
• Přidán litevský překlad.
• Přidán čínský překlad.
• Od této chvíle budou v sekci vydání projektu pravidelně zveřejňovány časté beta verze, aby uživatelé mohli testovat nové změny před další stabilní verzí.
• Přidána zkratka Ctrl+tečka pro vložení znaku výpustky (…).
• Vylepšena podpora kapitol podcastů: navigace mezi kapitolami nyní funguje spolehlivěji, včetně přímých/streamovaných epizod, kde kapitoly nejsou vložené v souboru MP3, díky použití záložních metadat kapitol z feedu/URL, pokud jsou k dispozici. Přidány zkratky pro navigaci mezi kapitolami Ctrl+Alt+PageUp (předchozí kapitola) a Ctrl+Alt+PageDown (další kapitola).
• Přeskupeny výstupní složky Sonarpad do Documents\Sonarpad: soubory jsou nyní ukládány do vyhrazených podsložek audiobooks, documents, recordings a media, s automatickou migrací ze starších cest.
• Vylepšena podpora velmi velkých textových souborů (včetně 60 MB): plynulejší otevírání a navigace po řádcích, zejména se čtečkami obrazovky.
• Aktualizovány příručky pro všechny jazyky a obnoveny lokalizační zdroje napříč aplikací, včetně textů pro dary a překladů instalátoru NSIS (nové řetězce instalátoru pro zjednodušenou čínštinu a litevštinu, plus dokončený ukrajinský překlad instalátoru).
• Přidána globální podpora síťové proxy (HTTP/HTTPS a SOCKS5/SOCKS5H) pro online funkce, s ověřením proxy při ukládání v Možnostech: neplatná proxy jsou oznámena a automaticky odstraněna.
• Přidána nová akce v nabídce Nástroje: „Přehrát streamované audio...“, která umožňuje vložit URL (YouTube nebo přímý odkaz na médium), zvolit výstupní formát a kvalitu a přehrát jej přímo v audio přehrávači Sonarpad.
• Přidána podpora systémové klávesy Přehrát/Pozastavit média (sluchátka/klávesnice): nyní ovládá jak přehrávání médií, tak pozastavení/obnovení čtení textu (s prioritou přehrávání médií, pokud jsou aktivní obě).
• Přidána nová položka v Soubor > Nedávné soubory: „Vymazat nedávné soubory“ pro rychlé smazání seznamu posledních dokumentů.
• Rozšířeny možnosti bitrate v Převést audio a v nastavení nahrávání podcastů: přidány nižší hodnoty (64/96 kbps) a MP3 rozšířeno až na 320 kbps, s odpovídající validací a zpracováním v enkodéru.
• Rozšířeny možnosti rozdělení audioknih podle času až na 60 minut.
• Vylepšeno rozdělení audioknih podle částí: uživatelé nyní mohou ručně zadat počet částí, s validací od 1 do 100.
• V nabídce Zobrazení byl přidán nový režim „Jen pro čtení“, který uzamkne editor proti nechtěným úpravám a přitom ponechá dokumenty plně čitelné a procházetelné.
• Během aktualizací programu přidán přístupný ukazatel průběhu, aby čtečky obrazovky mohly v reálném čase sledovat průběh stahování.
• Přidán nový tichý stavový řádek v hlavním okně zobrazující znaky, slova a řádek/sloupec (například: „Znaky (včetně mezer): 11. | Slova: 2. | Řádek 1, sloupec 12“) bez narušení zaměření NVDA.
• Přidána nová položka „Zalamovat řádky“ v nabídce Zobrazení, takže zalamování lze rychle měnit bez otevírání Možností.
• Přidány nové akce v nabídce Úpravy > Text: „Zvětšit odsazení řádku/bloku“ a „Zmenšit odsazení řádku/bloku“ se zkratkami Ctrl+Shift+. (odsadit) a Ctrl+Shift+, (zmenšit odsazení), protože když je zapnuto „Zobrazit hlasy v editoru“, klávesa Tab je vyhrazena pro navigaci v panelu hlasů.
• Přidáno lokalizované datum/čas v RSS článcích a epizodách podcastů, s formátováním přizpůsobeným aktuálnímu jazyku rozhraní.
• Přidána nová akce v kontextovém menu RSS pro sdílení vybraného článku e-mailem.
• Přidány podrobné možnosti potvrzení mazání pro RSS a podcasty v Možnosti > RSS a podcasty: RSS (feed/článek/oboje/žádné) a Podcasty (podcast/epizoda/oboje/žádné).
• Přidáno nastavitelné rychlé kopírování RSS pomocí Ctrl+C (Možnosti > RSS a podcasty): kopírovat titulek, URL, obsah článku nebo vše dohromady.
• Sjednoceno vytváření RSS zdrojů: „Přidat zdroj“ nyní přijímá jak přímé URL feedu, tak zadání klíčového slova (automaticky generuje Google News RSS), čímž nahrazuje potřebu samostatné akce pro vyhledávání podle klíčového slova.
• Stisknutí Ctrl+A nyní oznámí dokončení pro jasnější zpětnou vazbu čtečkám obrazovky.
• Přidána klávesa Shift+F3 pro „Najít předchozí“ v menu Úpravy, jako doplněk k F3 „Najít další“.
• Vylepšeny zprávy zpětné vazby při nahrazování se správnými tvary jednotného a množného čísla (např. „Provedena 1 náhrada“ vs „Provedeny 2 náhrady“).
• Přidán výběr jazyka pro slovník ve slovníkovém okně, s výchozí volbou Auto (jazyk rozhraní) a volitelným ručním nastavením.
• Přidána nová karta Zkratky v Možnostech pro přizpůsobení klávesových zkratek s detekcí konfliktů, která upozorní, pokud je zkratka již přiřazena jiné akci.
• Přidána počáteční podpora přepínačů příkazové řádky: -h/--help nyní zobrazují informace o použití a --version vypíše verzi programu.
• Vylepšena srozumitelnost ručního nastavování rychlosti a výšky hlasu: ruční pole nyní používají stupnici se středem 100, kde 100 odpovídá normální hodnotě.
• Vylepšen výběr hlasů Microsoft v Možnosti > Hlas i v panelu Hlas v editoru: přidán lokalizovaný jazykový seznam pro filtrování hlasů podle jazyka, přičemž režim pouze vícejazyčných hlasů zůstává jako jediný neseskupený seznam hlasů (jazykový seznam je při jeho zapnutí skryt).
• Přidána konfigurace hlasu dialogů v Možnosti > Hlas s plnou navigací pomocí Tab, používající stejný systém TTS jako hlavní rozhraní (systém TTS, jazyk hlasu, hlas a ruční ladění hlasu); přidán volitelný druhý hlas dialogů se stejnými ovládacími prvky pro střídající se dialogy; pravidla pro hlasy dialogů jsou ukládána do konfiguračního .ini, takže text dokumentu není upravován.
• Vylepšeno označení Zpět: položka Úpravy > Zpět nyní zobrazuje, jaká akce bude vrácena zpět (například úpravy textu, citovat/odcitovat řádky nebo vložení hlasového tagu), přičemž zůstává zakázaná, když není co vracet.

Opravy
• Opraveno otevírání souborů RTF: dokumenty .rtf jsou nyní parsovány a zobrazovány jako obyčejný čitelný text namísto surového RTF zápisu (např. {\\rtf1...}).
• Opraveno otevírání čínských textových souborů kódovaných v GB18030/GBK: Sonarpad nyní tyto soubory správně rozpozná a dekóduje, čímž se zabrání zkomolenému výstupu.
• Vylepšeno vytváření audioknih M4B s metadaty kapitol a značkami kapitol; opraven problém „chipmunk“ přehrávání (vysoká výška/rychlost) u vygenerovaných souborů M4B.
• Opraveno uživatelské rozhraní bitrate v dialogu ukládání audioknih: odstraněny natvrdo vložené italské popisky a přidána možnost 64 kbps mezi volitelné bitrate.
• Opraveno Uložit vše (Ctrl+Shift+S): všechny otevřené upravené dokumenty jsou nyní spolehlivě rozpoznány (včetně neuložených/nových karet) a Uložit vše správně uloží každý dokument nebo otevře Uložit jako, pokud je to potřeba.
• Opraveno řazení položek Google News RSS: články jsou nyní zobrazovány podle data publikace sestupně (nejnovější první), pokud jsou data dostupná.
• Opravena asociace popisků pro NVDA ve slovníkovém okně: pole hledání a jazykový seznam nyní oznamují správné popisky.
• Opraveno ovládání klávesnice v okně Vlastnosti RSS/Podcast: Tab/Shift+Tab nyní dosáhne na tlačítko OK, Enter aktivuje OK, Esc bezpečně zavře okno a zaměření se správně vrací do seznamu RSS/Podcast.
• Opravena historie vrácení změn RSS/Podcast: Ctrl+Z nyní podporuje víceúrovňové vracení odstranění (článků/epizod i zdrojů), nejen poslední akci.
• Vylepšena zpětná vazba při odstraňování RSS/Podcastů pomocí výslovných stavových oznámení (RSS odstraněno, RSS článek odstraněn, epizoda podcastu odstraněna).
• Vylepšeno chování fokusu RSS/Podcast po smazání/vrácení: RSS nyní spolehlivě zaostří první feed, když je to potřeba, a vyhýbá se opakovaným oznámením čtečky obrazovky při zpožděném znovuvýběru.

Verze 0.6.6 – 2026-02-13

Vylepšení
• Přidána možnost „Automatické formátování pro TTS“ v menu Úpravy pro rychlou přípravu textu pro řeč (odstraní markdown/uvozovky a znovu spojí zalomené řádky).
• Vylepšeno vkládání hlasových tagů: pokud je text vybrán, tagy se nyní správně použijí jak na výběr v jednom řádku, tak na víceřádkový výběr.
• Přidána možnost výchozí složky pro ukládání audioknih v nastavení Audio (výchozí: Documents\Sonarpad Audiobooks).
• V dialogu ukládání audioknih při zapnutém rozdělování přidána nová výchozí možnost pro vytvoření samostatné podsložky pro rozdělené části (pro přehlednější organizaci výstupu).
• Export audioknih nyní ukládá MP3 ve stereu s uživatelem zvoleným bitrate pro hlasy Edge, SAPI5 a SAPI4.
• Přidána podpora 32bitových hlasů SAPI5 přes bridge, takže hlasy dostupné pouze v 32bitových enginech lze také použít v Sonarpad.
• Funkce hlasu byly přesunuty do samostatné nabídky „Hlas a zvuk“ a byla přidána/upřesněna funkce „Převést audio...“, užitečná pro převod jakéhokoli podporovaného mediálního souboru do MP3, AAC (M4A), OGG (Vorbis), Opus, FLAC, WAV a AIFF.
• Přidáno odstraňování jednotlivých RSS článků a epizod podcastů (klávesa Delete + kontextové menu s potvrzením), aniž by byl odstraněn celý RSS/podcastový zdroj, plus vrácení posledního odstranění (jediný článek/epizoda nebo celý RSS/podcastový zdroj).
• Přidán export RSS zdrojů do OPML v RSS okně, takže aktuální RSS zdroje lze snadno uložit a znovu importovat.
• Přidána funkce „Hledat RSS podle klíčového slova“ v RSS okně: zadání klíčového slova nyní automaticky vygeneruje URL Google News RSS a otevře dialog přidání zdroje s předvyplněnými údaji, takže lze feedy podle klíčových slov vytvářet v jednom kroku.
• Přidán srbský překlad díky Mila Kuran.
• Přidán ukrajinský překlad díky Ivan Shtefuriak.
• Přidáno otevírání více mediálních souborů najednou: výběr/otevření více mediálních souborů nyní vytvoří frontu přehrávání místo nahrazení aktuálního souboru.
• Přidány zkratky pro proměnlivé posouvání během přehrávání: se základním skokem 1 minuta posouvají Left/Right o 60 s, Shift+Left/Right o 20 s a Ctrl+Left/Right o 3 minuty.
• Přidány zkratky pro předchozí/další stopu v přehrávači: Ctrl+PageUp a Ctrl+PageDown.
• Přidána funkce „Normální hlasitost (100%)“ a seskupeny obnovovací akce do samostatného podmenu „Reset“ v Přehrávání vedle „Normální rychlost (1x)“ a „Normální výška (0)“.
• Vylepšení instalátoru: setup.exe nyní umožňuje uživatelům zvolit mezi přiřazením všech podporovaných typů souborů nebo ručním výběrem přípon; MSI nyní nabízí volby asociací souborů po jednotlivých příponách ve stromu funkcí (výchozí zůstává vše povoleno).
• Přidána nová nabídka „Okno“ s položkou „Otevřít dokumenty...“ pro rychlé přepnutí na libovolný aktuálně otevřený soubor.
• Aktualizováno Zobrazení > Písmo: starý výběr byl nahrazen rychlou podnabídkou běžných písem (Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman, Georgia) při zachování aktuální velikosti textu.
• Vylepšena oznámení RSS/Podcastů pomocí duálního modelu stavu: uzly zdrojů oznamují „nové položky“, když má zdroj nebo podcast aktualizace, zatímco jednotlivé RSS články a epizody podcastů oznamují „nepřečteno“ / „nepřehráno“; toto chování lze vypnout v Možnostech.

Opravy
• Opravena extrakce textu z EPUB pro knihy obsahující vložené HTML komentáře (`<!-- ... -->`): text kapitol je nyní parsován správně místo částečného nebo úplného přeskočení.
• Opraveno vyhledávání ve španělském Wiktionary a zpracování cache slovníku: španělské položky jako „agua“ se nyní načítají správně a staré záznamy cache „Slovo nenalezeno“ se již znovu nepoužívají.
• Opraveno kódování znaků při importu RSS článků z některých španělských zdrojů (např. El Mundo): písmena s diakritikou a „ñ“ jsou nyní správně zachována v dočasném editoru.
• Opraveno dekódování ANSI textu pro středoevropské soubory (např. čeština/polština): Sonarpad nyní lépe rozlišuje UTF-8 vs ANSI a vybírá správnou kódovou stránku (včetně Windows-1250), aby se zabránilo poškození diakritiky.
• Opravena perzistence RSS zdrojů pro feedy s parametry dotazu v URL (např. rss.aspx?c=...): tyto feedy se nyní po restartu Sonarpad správně ukládají a obnovují.
• Opraveno otevírání souborů ukazatelů Google Drive (.gdoc, .gsheet, .gslides) z kontextového menu Průzkumníka: když přímé čtení selže s chybou „Incorrect function (os error 1)“, Sonarpad nyní použije shell-open, aby se dokument přesto správně otevřel.
• Opraveno čtení starých souborů Excel 2010 .xls: staré binární excelové soubory jsou nyní správně rozpoznány a dekódovány místo zobrazení zkomoleného textu (např. ÐÏ_à¡±...).
• Opraven průběh oznamování kontroly pravopisu: chybně napsaná slova jsou nyní znovu oznamována při pozdější kontrole textu a stejná chyba je znovu nahlášena, pokud je smazána a znovu napsána.
• Opraveny textové akce založené na řádcích (např. Ctrl+Q / Ctrl+Shift+Q, řazení/obrácení/jedinečné/sloučení řádků): výběr jednoho řádku pomocí Shift+Down již neslučuje ani nezkracuje sousední řádky.
• Opraveno víceřádkové chování pro textové akce založené na řádcích (Ctrl+Q / Ctrl+Shift+Q a související nástroje): výběry RichEdit používající oddělovače pouze CR jsou nyní správně normalizovány, takže všechny vybrané řádky jsou zpracovány bez oříznutí prvních znaků.
• Rozšířena normalizace vstupu TTS pro viditelné symboly bílých znaků (␠/U+2420, ␣/U+2423, ␉/U+2409, ␊/U+240A, ␍/U+240D, ␤/U+2424), aby se zabránilo opakovanému přehrávání odstavců u vícejazyčných hlasů.
• Upřesněna sanitizace textu Edge TTS pomocí jediného validačního řetězce: zvláštní/neviditelné mezery jsou normalizovány, dlouhé sekvence interpunkce (např. "...", "!!!", "???") jsou zkráceny a úseky obsahující pouze interpunkci jsou přeskočeny, aby se zabránilo smyčkám přehrávání.
• Opraveno oznamování času přehrávání (Ctrl+I) pro streamy MP3/podcastů: aktuální čas je nyní omezen délkou stopy a přehrávání se automaticky zastaví, pokud pozice překročí konec.
• Vylepšeno pokrytí lokalizace instalátoru: setup.exe nyní obsahuje další jazyky instalátoru (čeština, polština, francouzština, srbština), zatímco MSI zůstává jako jediný balíček en-US, aby se předešlo zmatku při vydání.
• Opraveno vyčištění při odinstalaci pro položky kontextového menu: „Otevřít v Sonarpadu“ je nyní spolehlivě odstraněno, včetně starších scénářů registru.
• Opravena spolehlivost pozastavení/obnovení u SAPI5: F4 nyní správně pozastaví a obnovení pokračuje z očekávané pozice místo restartu od začátku.
• Opraven průběh pozastavení + posun + obnovení pro přehrávání médií: po pozastavení a posunu pomocí Left/Right nyní stisknutí Space spolehlivě pokračuje z aktuální pozice místo zastavení nebo restartu od začátku.

Verze 0.6.5 – 2026-02-07

Vylepšení
• Vylepšen španělský překlad díky Arturo Fernandez Rivas.
• Přidána možnost rozdělit EPUB audioknihy podle kapitol.
• Importy RSS nyní používají vyhrazenou dočasnou kartu (lokalizovaný název); Uložit jako ji převede na běžný dokument.
• Zprávy pro čtečky obrazovky jsou nyní při dostupnosti posílány také do JAWS.

Opravy
• Čtení od kurzoru (F5) nyní začíná přesně na pozici kurzoru. Dříve mohlo začínat o několik řádků výše, protože posun kurzoru neodpovídal pozicím CRLF/UTF-16.
• Opraven problém s překreslováním, kdy psaní přes výběr mohlo způsobit dočasné zmizení dřívějšího textu, dokud se výběr neposunul.
• Opraveno parsování kapitol EPUB, takže stránky pouze s obálkou nebo obrázkem již nevedou k předčítání CSS (např. „padding“) nebo názvům „Neznámý“.
• Opraveno rozdělení audioknih podle času z EPUB s Edge TTS, které selhávalo na prázdných/příliš velkých úsecích („Edge audio not sent“).
• RSS články nyní dekódují HTML entity (např. `&quot;`, `&amp;`, `&lt;`, `&gt;`).
• Uložit/Uložit jako nyní navrhuje existující název souboru při ukládání nepřepisovatelných formátů (např. EPUB) místo prvního řádku.
• Opraven problém, kdy podcasty s novými epizodami nebyly oznamovány jako nepřehrané, a „Unheard“ bylo přejmenováno na „Unplayed“ pro profesionálnější označení.

Verze 0.6.4 – 2026-02-05

Vylepšení
• Program byl přejmenován na Sonarpad, aby zdůraznil zvuk a audio jako hlavní zaměření.
• Přidán výběr zvukové stopy v menu Přehrávání pro mediální soubory s více zvukovými stopami (např. MKV soubory s více jazyky).
• Podcasty nyní jasně označují nepřehrané epizody předponou „Nepřehráno“ před názvem.
• Nové přepínání hlasů v textu pomocí tagů. Příklady:
  - Hlasy Microsoft (Edge): <voice edge it-IT-IsabellaNeural>Hello</voice>
  - Hlasy SAPI5: <voice sapi5 Microsoft Helena Desktop>Hello</voice>
  - Hlasy SAPI4: <voice sapi4 #1>Hello</voice>
  - Se změnou rychlosti/výšky/hlasitosti: <voice edge it-IT-ElsaNeural speed=-20 pitch=-5 volume=-10>Hello</voice>
• Rozšířené kategorie podcastů.
• Vylepšené čtení PDF s automatickým přepnutím na PDFium.
• Vylepšen parser článků pro případy, kdy se obsah nenačetl celý.
• Přidáno resetování výšky hlasu (pitch) v menu Přehrávání.
• Přidána možnost v kontextovém menu „Vytvořit audioknihu z výběru“.
• Přidáno rozdělení audioknihy podle délky, s možností zvolit název prvního souboru.
• Lokalizován štítek autora při čtení článků (např. „by“, „di“, „par“).
• Přidány možnosti odsazení (tabulátory/mezery s nastavením šířky) a odsazení/odsazení zpět pomocí Tab/Shift+Tab na vybraných řádcích.
• Opraveno čištění Markdownu pro správné zpracování odrážek „*“, když je zachování odrážek vypnuto.
• Přidána možnost používat starý název „Novapad“ v titulku okna a ve zkratkách nabídky Start.

Opravy
• Opraven problém, kdy audioknihy SAPI4 byly vytvářeny jinak, než se očekávalo.
• Opraven problém, kdy posun za konec mediálního souboru znovu spustil přehrávání od začátku.
• Okno Najít v souborech: stisknutí Enter na výsledku nyní otevře správnou pozici a Esc vrátí zpět na výsledky.
• Okno Možnosti: vylepšené rozložení na kartách Obecné, Hlas, Editor a Audio, aby se zabránilo chybějícím nebo oříznutým prvkům.
• Opraven problém se záložkami při změně rychlosti přehrávání.
• Opraveno zobrazování kategorií Podcast Index.
• Opraven problém s apostrofy, které narušovaly čtení – odstraněno oddělené čtení dialogů, místo toho se používají voice tagy.

Verze 0.6.3 – 2026-01-30

Vylepšení
• Vylepšena detekce mikrofonu.
• Přidána podpora okamžitého přehrávání pro všechny formáty.

Opravy
• Opraven pád aplikace v okně kategorií podcastů.

Verze 0.6.2 – 2026-01-30

Nové funkce
• Přidána podpora spouštění souborů (Shift+F5). Uživatelé mohou v Možnostech zvolit interpret (např. python), vyhledat ho v počítači a stisknutím Shift+F5 spustit aktuální skript. HTML soubory se otevírají v prohlížeči.
• Přidána podpora odkazových souborů Google Docs (.gdoc, .gsheet, .gslides), které se automaticky otevřou ve výchozím prohlížeči.
• Přidána podpora formátu audioknih M4B (Apple/AAC).
• Přidána možnost „Zobrazit epizody“ v kontextovém menu výsledků vyhledávání podcastů pro procházení a přehrávání epizod bez odběru.
• Přidána funkce „Přejít na řádek“ (menu Úpravy nebo Ctrl+J).
• Přidány možnosti v kontextovém menu pro řazení RSS a podcastů (abecedně nebo podle data).
• Přidány výchozí RSS kanály pro vietnamštinu.
• Přidáno testovací pole mikrofonu v dialogu nahrávání.
• Přidána možnost „Zobrazit popis“ epizod podcastů v kontextovém menu.
• Přidána podpora rozšířených audio/video formátů přes FFmpeg: mkv, avi, mov, m4v, webm, mpg, ts, wmv, flv, vob, 3gp, flac, ogg, wma, aiff.
• Přidána podpora synchronizovaného čtení titulků (srt, vtt, ass, sub, sbv, lrc, smi) pomocí NVDA nebo zvoleného hlasu. Program hledá soubor titulků se stejným názvem jako mediální soubor. Přidány možnosti „Přidat titulky...“ a „Odebrat načtené titulky“ v menu Přehrávání.
• Přidány asociace souborů pro všechny nové podporované formáty v menu „Otevřít v Sonarpadu“.
• Přidáno nastavení výšky hlasu (pitch) pro jakýkoli soubor.
• Přidána možnost v obecném nastavení zapnout nebo vypnout anonymní hlášení chyb. Přidána položka v menu Nápověda pro vytvoření diagnostického ZIP souboru.
• Přidána možnost použít jiný hlas pro dialogy, jak při živém čtení, tak při tvorbě audioknih.
• Přidán prohlížeč kategorií podcastů.

Vylepšení
• Otevření audio/video souboru z Průzkumníka nyní otevře přímo přehrávač místo textového editoru.
• Odstraněn dotaz OCR pro nepřístupné PDF – OCR se nyní provádí automaticky.
• Vylepšen Přístupný terminál – NVDA si pamatuje poslední přečtený řádek.
• SAPI4: tvorba audioknih je nyní paralelní a téměř okamžitá.
• SAPI4: odstraněno úzké hrdlo převodu WAV→MP3 díky paralelnímu zpracování.
• SAPI4: vylepšené zpracování chyb a čištění dočasných souborů.
• V dialogu hledání bylo „Regex“ přejmenováno na „Regular expression“.
• M4B audioknihy: lepší práce s výstupem a kapitolami.
• Přehrávač: opraveny záložky a čas při jiné rychlosti než 1.0x.
• Obnovena navigace Ctrl+Tab a Ctrl+Shift+Tab v Možnostech.
• Přidána možnost rychlého resetu rychlosti na 1.0x.
• Aktualizovány všechny závislosti.
• Integrovaný FFmpeg s dynamickým načítáním DLL.
• Aktualizovány filtry stahování podcastů.
• Zabráněno ukládání audio/video souborů pomocí Ctrl+S.
• Vylepšen import YouTube transkriptů.
• Vylepšeno dělení audioknih bez ztráty textu.
• Instalátor je nyní vícejazyčný.
• Kategorie podcastů: Enter nyní potvrzuje výběr.
• Vylepšen systém detekce zamrznutí.

Opravy
• Opraven problém, kdy se changelog neotevřel při spuštění.
• Opraven problém s OCR při otevření PDF z Průzkumníka.
• Opraven problém při startu způsobující ztrátu zaměření nebo zavření okna.
• Opraven kritický problém v regex hledání (Wrap around, Dot matches newline).

Lokalizace
• Přidán polský překlad.
• Přidán francouzský překlad.
• Přidán český překlad (díky Radek Žalud a Jiri Holzinger).

Verze 0.6.1 – 2026-01-20

Opravy
• Opraven problém, kdy zapnutí „Zobrazit hlasy v editoru“ způsobovalo zastavení přehrávání podcastu.
• Opraven problém, kdy některé podcasty nešlo přidat pomocí URL, protože URL byla zkrácena.
• Opraven problém, kdy běžné URL již nešlo přidat ve funkci RSS kanálů.
• Opraven problém, kdy se možnost jazyka Wikipedie zobrazovala vícekrát v různých kartách nastavení.
• Odstraněno vytváření ladicích souborů, které se chybně generovaly i v produkčním režimu.

Vylepšení
• Vylepšená podpora hlasů Microsoft, které nyní používají speciální metodu přehrávání s odlišným user agentem.
• Přidána podpora souborů MP4.

Verze 0.6.0 – 2026-01-20

Nové funkce
• Přidána kontrola pravopisu. V kontextové nabídce mohou uživatelé zkontrolovat, zda je aktuální slovo správné, a pokud ne, získat návrhy oprav.
• Přidán import a export podcastů pomocí souborů OPML.
• Přidána podpora vyhledávání Podcast Index vedle iTunes. Uživatelé mohou zadat svůj bezplatný API klíč a tajný klíč (generovaný pouze pomocí e-mailu).
• Přidána podpora hlasů SAPI4 pro čtení v reálném čase i tvorbu audioknih.
• Byla přidána automatická podpora OCR pro nepřístupné PDF: pokud není nalezen extrahovatelný text, dokument je rozpoznán pomocí OCR.
• Přidána podpora slovníku pomocí Wiktionary. Stisknutím klávesy Applications se zobrazí definice a pokud jsou dostupné, také synonyma a překlady do jiných jazyků.
• Přidán import článků z Wikipedie s vyhledáváním, výběrem výsledků a přímým importem do editoru.
• Přidána zkratka Shift+Enter v RSS modulu pro otevření článku přímo na původní webové stránce.

Vylepšení
• Výběr mikrofonu je nyní vždy respektován aplikací.
• V okně podcastů nyní stisknutí Enter na epizodě okamžitě oznámí „načítání“ přes NVDA pro potvrzení akce.
• Ve výsledcích vyhledávání podcastů nyní Enter odebírá vybraný podcast.
• Opraveny a vylepšeny popisky pro zkratky Ctrl+Shift+O a Ctrl+Shift+P (Podcast).
• Rychlost přehrávání a hlasitost jsou nyní ukládány v nastavení a platí pro všechny audio soubory.
• Přidána speciální složka cache pro epizody podcastů. Uživatelé mohou epizody uchovat pomocí „Zachovat podcast“ v menu přehrávání. Cache se automaticky čistí při překročení uživatelem definované velikosti (Možnosti → Audio).
• Výrazně vylepšeno načítání RSS článků pomocí libcurl impersonation s profily Chrome a iPhone, což zajišťuje kompatibilitu s ~99 % webů.
• Přidán stav přečteno / nepřečteno pro RSS články s jasným označením v seznamu.
• Funkce Nahradit vše nyní hlásí počet provedených nahrazení.
• Přidáno tlačítko Smazat podcast při navigaci v knihovně podcastů pomocí Tab.

Opravy
• Odstraněna redundantní položka „Čekající aktualizace“ z menu Nápověda (aktualizace jsou již řešeny automaticky).
• Opraven problém, kdy stisknutí Ctrl+S na otevřeném MP3 souboru způsobilo jeho poškození.
• Opraven problém UI, kde „Hromadné audioknihy“ bylo zobrazeno jako „(B)… Ctrl+Shift+B“.
• Opraveny chytré uvozovky: při zapnutí se nyní správně nahrazují běžné uvozovky.
• Opraven problém, kdy „Přejít na záložku“ resetovalo rychlost přehrávání na 1.0.
• Opraven problém, kdy již stažené epizody podcastů byly znovu stahovány místo použití cache.

Klávesové zkratky
• F1 nyní otevře nápovědu.
• F2 nyní zkontroluje aktualizace.
• F7 / F8 nyní přechází na předchozí / další pravopisnou chybu.
• F9 / F10 nyní rychle přepíná mezi oblíbenými hlasy.

Vylepšení pro vývojáře
• Chyby již nejsou tiše ignorovány: všechny vzory let _ = byly odstraněny a chyby jsou nyní explicitně řešeny.
• Projekt nyní selže při kompilaci, pokud existují varování.
• Odstraněny vlastní implementace strlen / wcslen.
• Zpracování DLL bylo sjednoceno s využitím knihovny libloading.
• Odstraněno ruční parsování bajtů, nyní se používají standardní metody.
Tyto změny zvyšují robustnost, bezpečnost a udržovatelnost kódu.

Verze 0.5.9 - 2026-01-13

Nové funkce
• Přidáno řazení RSS v kontextové nabídce (nahoru/dolů/na pozici).
• Přidána nabídka článku: otevřít v prohlížeči a sdílet přes WhatsApp, Facebook a Twitter/X.
• Přidána zkratka Esc pro návrat z článku do RSS seznamu.
• Přidán režim podcastů: vyhledávání, odběr, poslech.
• Přidána kontrola rychlosti přehrávání.
• Přidáno Ctrl+T pro skok na čas.
• Přidáno tlačítko náhledu hlasu.
• Přidáno hledání a nahrazování pomocí regexu.
• Přidán import RSS z OPML a TXT.
• Přidána možnost „Otevřít v Sonarpadu“ do kontextové nabídky.

Vylepšení
• Vylepšeno ovládání hlasu.
• Vylepšeno RSS bez změny zaměření NVDA.
• Vylepšeno audio přehrávání.
• Přidány chybějící zkratky.
• Reorganizováno menu Úpravy.
• Reorganizovány Možnosti do karet.
• RSS nyní načítá celý obsah článku.

Opravy
• Opraveno odstraňování čísel v Markdownu.
• Opraven AltGr+Z (undo).
• Opraveno zrušení nahrávání audioknihy.

Lokalizace
• Přidán vietnamský překlad.

Verze 0.5.8 - 2026-01-10

Nové funkce
• Přidáno ovládání hlasitosti mikrofonu a systému při nahrávání podcastů.
• Přidán import článků z webů a RSS.
• Přidáno odstranění všech záložek.
• Přidáno odstranění duplicitních řádků.
• Přidáno zavření všech oken kromě aktuálního.
• Přidána položka „Přispět na vývoj programu“ v nabídce Nápověda.

Vylepšení
• Vylepšen přístupný terminál.
• Opraveny zkratky.
• Opraveno přehrávání po zavření okna.
• Přidána potvrzení akcí.
• Přidáno mazání RSS pomocí Delete.
• Přidáno menu pro úpravu RSS.
• Odstraněno ruční nastavení složky konfigurace (nyní automatické).

Verze 0.5.7 - 2026-01-05

Nové funkce
• Přidána funkce Hromadné audioknihy.
• Přidána podpora Markdown (.md).
• Přidán výběr kódování souboru.
• Přidáno oznamování nových řádků NVDA.

Vylepšení
• Audioknihy se ukládají přímo do MP3.
• Nastavitelná pozice hvězdičky změn.
• Vylepšen systém aktualizací.
• Přidáno odstranění spojovníků.

Verze 0.5.6 - 2026-01-04

Opravy
• Vylepšeno Najít v souborech.

Vylepšení
• Přidána podpora PPT/PPTX.
• Netextové formáty se ukládají jako .txt.
• Přidáno nahrávání podcastů.

Verze 0.5.5 – 2026-01-03

Nové funkce
• Přidán přístupný terminál.
• Přidán přenosný režim.

Opravy
• Vylepšeno Najít v souborech.

Verze 0.5.4 – 2026-01-03

Vylepšení
• Opravena funkce „Normalizovat mezery“.
• Přidána podpora souborů HTML.

Verze 0.5.3 – 2026-01-02

Nové funkce
• Přidáno Najít v souborech.
• Přidány nástroje pro text.
• Přidána statistika textu.
• Přidány příkazy pro seznamy.
• Přidány funkce „Citovat řádky“ a „Zrušit citaci řádků“.

Lokalizace
• Přidána španělština.
• Přidána portugalština.

Vylepšení
• EPUB se ukládá jako .txt.

Verze 0.5.2 - 2026-01-01
• Přidán seznam změn.
• Přidány asociace souborů.
• Vylepšena lokalizace.
• Přidáno dělení audioknih.
• Přidán import YouTube transkriptů.

Verze 0.5.1 - 2025-12-31
• Automatické aktualizace.
• Vylepšení audioknih.
• Vylepšení TTS.
• Menu Zobrazení a panely.
• Lokalizace.
• CI a balíčkování.

Verze 0.5.0 - 2025-12-27
• Modulární refaktor.
• Workflow pro sestavení Windows verze.
• Oprava TAB v nápovědě.

Verze 0.5 - 2025-12-27
• Předběžné zvýšení verze.

Verze 0.1.0 - 2025-12-25
• První vydání: struktura projektu a README.

10. Fixed segment reanalysis timing when mapping an independently analyzed mini-film back to the original movie. Sonarpad now applies the same small chunk-duration reconciliation used by the normal full analysis to descriptions, visual-evidence timestamps, and protected dialogue intervals, preventing progressive sub-second drift near the end of a chunk from moving narration onto speech.

