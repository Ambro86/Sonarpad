# Dnevnik izmena

Verzija 0.9.9 – 2026-09-15

AI audio opis
1. Dodat je izolovani rezervni put za izvorne video datoteke bez audio zapisa, bez menjanja postojećeg funkcionalnog toka. Sonarpad i dalje prvo pokušava normalan izvoz; samo ako FFmpeg prijavi da audio tok nedostaje i Sonarpad potvrdi da izvor zaista nema nijedan audio zapis, kreira se tišina iste dužine i na nju se miksuju sintetizovani opisi. Pri izvozu na originalni video zatim se ponovo koristi postojeći mux put za video+audio. Video datoteke koje već imaju audio nastavljaju tačno normalnim putem.

2. Проширен је строго изоловани резервни режим за случај када нема употребљивих описа, без измене нормалног тока аудио-дескрипције. Sonarpad и даље прво покреће постојећу анализу; само ако се она заврши без употребљивих описа нуди се други покушај у режиму Кратко, који користи исти ток и задржава сва ограничења тишине и забрану преклапања са дијалогом. Ако ни други покушај не може да уметне опис, Sonarpad сада тражи изричиту сагласност за последњи резервни режим. Тек након избора Да поново се користе кратки описи које је Gemini већ генерисао и који су сачувани у checkpoint-у другог покушаја, па се миксују у својим визуелним временима уз ducking и дозвољена кратка преклапања са дијалогом. Pyannote, Gemini bridge, нормална правила поравнања, TTS распоређивање и прва два пута анализе остају непромењени. Ако корисник одбије или нема описа за поновну употребу, Sonarpad се коректно зауставља уз локализовану поруку.

3. Unapređen je izolovani rezervni režim kada nema upotrebljivih opisa, bez menjanja normalne obrade audio-deskripcije. Ako je korisnik već na početku izabrao Kratke opise i analiza završi bez upotrebljivog opisa, Sonarpad sada preskače suvišnu drugu Gemini analizu i, kada postoje generisani opisi koji mogu da se ponovo upotrebe, odmah nudi završni fallback sa preklapanjem dijaloga. Ako je početna opširnost Standardna ili Detaljna, Kratki pokušaj se zadržava čak i kada su dostupne pauze veoma kratke: tada služi i za generisanje kraćeg opisa za mogući završni fallback, kako bi naracija što kraće prekrivala dijalog. Pyannote, Gemini bridge i normalna pravila tišine i poravnanja ostaju nepromenjeni.

Snimanje podkasta
1. Dodat je konzervativni fallback za pogrešne pozicije uređaja koje prijavljuju pojedini problematični WASAPI drajveri mikrofona, bez menjanja snimanja na sistemima na kojima već radi. Aktivira se samo posle trajnih nepodudaranja pozicije: 24 uzastopna ili najmanje 80% u 64 ispravna paketa. Pravi WASAPI `DATA_DISCONTINUITY` signali i greške vremenskih oznaka i dalje imaju prednost, a snimanje sistemskog zvuka ostaje nepromenjeno.

YouTube i striming
1. Ispravljena su neuspešna YouTube preuzimanja pokrenuta iz kontekstnog menija rezultata. Poznate yt-dlp greške za video-snimke dostupne samo članovima kanala sada se pretvaraju u lokalizovanu Sonarpad poruku umesto prikazivanja sirovog engleskog teksta. Posle zatvaranja greške Sonarpad vraća fokus tek kada se nativni kontekstni meni/modalna petlja stvarno završi, pa se pouzdano vraća na listu rezultata sa aktivnom tastaturom. Uobičajeni tok uspešnog preuzimanja ostaje nepromenjen.

2. Preventivni filter je usklađen sa mobilnim SonarTube-om: u rezultatima pretrage i pri pregledanju kanala/plejlista sada se skrivaju video-snimci za koje YouTube/yt-dlp ne vraća podatak o broju pregleda. Kanali i plejliste ostaju vidljivi, a stvarni video-snimci sa 0 pregleda se zadržavaju. Lokalizovana obrada greške za sadržaj samo za članove ostaje kao drugi rezervni mehanizam. Grupno preuzimanje plejlista nije promenjeno.

Verzija 0.9.8 – 2026-09-12

AI audio opis
1. Dodat je konzervativni oporavak višekanalnog izvoza bez menjanja pipeline-a za datoteke koje već rade. Sonarpad i dalje prvo koristi originalni raspored kanala i samo po potrebi dopunjava nepotpun poslednji PCM okvir. Ako višekanalni privremeni WAV ipak ne može da se završi zato što broj uzoraka nije deljiv brojem kanala, Sonarpad automatski ponavlja samo taj izvoz u stereo formatu. Uspešni mono, stereo i višekanalni izvozi ostaju nepromenjeni.

Tamna tema i pristupačnost
1. Ispravljeno je da tamna tema ometa izvorne Windows dijaloge i poruke za potvrdu. Sonarpad više ne primenjuje svoj prilagođeni dark-theme subclass na sistemska `#32770` stabla dijaloga, pa dijalozi Otvori/Sačuvaj, izvoz dijagnostike i MessageBox ostaju pod upravljanjem Windows-a i pravilno dobijaju fokus tastature/NVDA. Ispravljen je i životni vek podrazumevane ZIP ekstenzije u dijalogu za čuvanje dijagnostike.

Verzija 0.9.7 – 2026-09-11

AI audio opis
1. Додата је опциона могућност креирања аудио-дескрипције у оригиналној видео датотеци. Када је укључена, Sonarpad сада прво бира MP4: оригинални видео ток се копира без поновног кодирања и додаје се мешана трака аудио-дескрипције. Ако FFmpeg при креирању MP4 пријави некомпатибилност контејнера или уписа пакета, Sonarpad аутоматски понови исти извоз као MKV, и даље без поновног кодирања видеа. MKV се може изабрати и ручно. Продужене паузе се у овом режиму игноришу да би звук и видео остали синхронизовани.

2. Poboljšana je ponovna analiza segmenata u sačuvanim projektima audio-deskripcije. Prozor sada koristi AI pristup koji je već podešen u glavnom prozoru za kreiranje audio-deskripcije, pa se više ne dupliraju API ključ, Sonarpad AI kredit i izbor modela. Ponovo analizirani segment sada zadržava celu grupu opisa projekta koji pripadaju istom segmentu analize, umesto samo prvog ili najbližeg opisa; svi opisi u grupi dobijaju oznaku da su ponovo analizirani, a „Primeni segment i ponovo izvezi MP3“ primenjuje celu grupu odjednom. Pregledi produženih pauza sada se sintetišu direktno umesto traženja pozicije u izvezenom MP3 fajlu, čime se izbegava da pregled počne zvukom filma ili da odseče opis.

3. Poboljšan je konzervativni rezervni postupak za Gemini medije bez menjanja postojećeg toka koji već radi. HTTP 400 `INVALID_ARGUMENT` i dalje aktivira manje MKV segmente (oko 15 MB), a zatim kompatibilne MP4 segmente. Ako Gemini prihvati poslati segment, ali se obrada završi stanjem `FAILED`, Sonarpad ponovo šalje potpuno isti segment jednom i tek zatim prelazi na MP4; pokušaji za serverski Code 13 ograničeni su na tri pre istog fallbacka, čime se izbegava beskonačno čekanje. Mrežne, kvota, autentifikacione ili greške sinteze ne aktiviraju ovaj put.

4. Исправљено је чување AI приступних података при промени режима приступа. Лични Gemini API кључ и Sonarpad AI приступни код сада се чувају независно: прелазак између личног API кључа и Sonarpad AI више не брише неактивни приступни податак. Gemini кључ се такође чува када његово поље изгуби фокус.

5. Додато је право дугме Откажи за „Поново анализирај сегмент“ и „Примени сегмент и поново извези MP3“. Трака напретка остаје активна, а Откажи сада прекида FFmpeg припрему сегмента, AI worker, TTS проверу и завршни извоз чим текућа фаза може безбедно да се заустави. Примена поново анализираног сегмента сада је трансакциона: постојећи пројекат и излазни медиј замењују се тек после успешног поновног извоза; ако се операција откаже, претходне датотеке остају неизмењене, а предлог поновне анализе остаје доступан за нови покушај.

6. Ispravljena je ponovna analiza segmenata za opise izvan prvog Gemini segmenta. Izdvojeni segment se sada šalje postojećem worker-u sa lokalnom vremenskom linijom koja počinje od 0, a zatim se vraća na apsolutnu poziciju u projektu, čime se izbegava `Invalid prepared chunk timeline at chunk 1` bez promene normalnog toka audio-deskripcije.

7. Ponovna analiza segmenata u sačuvanim projektima sada koristi iste bezbednosne provere kao potpuno pravljenje audio-deskripcije. Izabrani segment prolazi kroz isto izdvajanje mono zvuka od 16 kHz i Pyannote analizu dijaloga/tišine, ista vremenska pravila i Gemini rezervne putanje, istu stvarnu TTS sintezu glasom projekta i isti završni raspoređivač za bezbedno postavljanje i produžene pauze. Pri primeni segmenta Sonarpad zadržava nova proverena apsolutna vremena i osvežava Pyannote zaštitu za taj segment; prethodno posebno zaobilaženje pregleda za predugačak ponovo analiziran tekst je uklonjeno.

8. Ispravljena je strukturna zamena segmenta posle potpune ponovne analize. Sonarpad više ne zahteva da novo provereni segment ima potpuno isti broj opisa kao sačuvani projekat: stari opisi tog segmenta zamenjuju se kompletnim bezbednim rezultatom pune obrade, pa segment sa 10 opisa može ispravno postati 9 (ili 11). Uređivač odmah prikazuje novi broj i nova vremena za pregled, dok sačuvani projekat i medijska datoteka ostaju nepromenjeni sve dok primena segmenta i ponovni izvoz MP3 ne budu uspešno završeni.

9. Ponovna analiza segmenta je prerađena tako da se izabrani fizički isečak tretira kao pravi samostalni mini-film. Sonarpad sada taj mini-film prosleđuje direktno potpuno istoj funkciji `create_audio_description` koja se koristi pri normalnom kompletnom kreiranju, pa video i audio zapis dele istu lokalnu vremensku osu, a uobičajene Pyannote, Gemini, stvarni TTS, raspoređivanje i bezbednosne provere rade bez posebne implementacije ponovne analize. Tek po završetku normalne obrade dobijena lokalna vremena se pomeraju nazad na originalnu poziciju u filmu.

Uređivanje teksta
1. Poboljšana je opcija „Spoji prelomljene redove“ u Uredi > Tekst. Bez izbora obrađuje ceo dokument, a sa izborom samo označene redove. Sada rekonstruiše cele pasuse spajanjem uzastopnih redova čak i kada se rečenica završava interpunkcijom; prazni redovi ostaju razdvajači pasusa, a numerisane i označene liste se čuvaju.

Verzija 0.9.6 – 2026-09-09

1. Ispravljeno je blokiranje nekih SAPI4 glasova kada je praćenje kursora uključeno. Bridge sada čeka stvarni završetak sinteze, dok praćenje kursora ostaje aktivno.

2. Ispravljeni su sinhronizacija mikrofona i sistemskog zvuka i pucketanje izazvano zaokruživanjem vremenskih oznaka pri snimanju podkasta. Ispravke važe i za odvojeno čuvanje izvora.

3. SAPI5 sinteza audio-opisa sada radi u zasebnim procesima. Posle greške pokušava ponovo sa manjim brojem paralelnih procesa, čuva uspešne opise i pamti odgovarajuće ograničenje za glas.

Verzija 0.9.5 – 2026-09-07

Аудио-дескрипција уз AI
1. Побољшана је отпорност када Gemini привремено врати `MALFORMED_RESPONSE` без употребљивог садржаја. Sonarpad сада поново покушава са потпуно истим сегментом до три пута, уз кратко чекање између покушаја, уместо да одмах прекине целу аудио-дескрипцију. Нормални Gemini одговори, безбедносне блокаде, грешке ограничења токена и постојеће понашање контролних тачака остају непромењени.

2. Ispravljen je još jedan redak slučaj greške `invalid Gemini chunk timeline` kod nekih video zapisa čiji segmenti pripremljeni pomoću FFmpeg-a prijavljuju malo kumulativno odstupanje trajanja. Sonarpad ostavlja postojeći tok nepromenjen kada je vremenska linija ispravna i koristi konzervativno usklađivanje samo kada bi normalna vremenska linija otkazala i ukupno odstupanje je malo; velika ili sumnjiva odstupanja i dalje izazivaju grešku.

3. Додата је опциона Sonarpad AI услуга за AI аудио-дескрипцију. Корисници могу и даље да користе сопствени Gemini API кључ или да изаберу Sonarpad услугу и унесу лични Sonarpad код. Код је заштићен помоћу Windows DPAPI. У режиму услуге припремљени видео сегменти се шаљу директно са корисниковог рачунара на Google преко привременог Google URL-а за отпремање; sonarpad.com не прима нити чува видео бајтове. Backend само одобрава приступ, намеће Gemini модел/Standard ниво и ограничења употребе, евидентира потрошњу и враћа Gemini одговор.

4. Побољшано је уређивање сачуваних пројеката аудио-описа. Сада је могуће изменити више описа један за другим без тренутног примењивања сваке измене. Измене остају у меморији; притиском на „Примени текст“ Sonarpad проверава сваки измењени опис у односу на његов сачувани временски опсег и затим заједно чува све исправне измене. Ако опис не може да стане у свој опсег, фокус се враћа на њега без губитка осталих измена на чекању.

5. Додато је „Подеси глас“ непосредно пре „Креирај аудио-опис“. Нови прозор омогућава избор мотора, гласа, брзине и јачине звука и садржи „Тестирај глас“. Притиском на OK фокус се враћа тачно на „Подеси глас“, а изабране вредности се примењују на аудио-опис који се креира. Ако се прозор не користи, синтеза остаје потпуно иста као раније.


6. Proširene su kontrole glasa u sačuvanim projektima audio-deskripcije. U izmeni projekta, pored motora i glasa, sada su dostupni brzina, jačina zvuka i „Testiraj glas“. Kada se primene parametri glasa, Sonarpad ponovo proverava sve postojeće opise sa novim parametrima sinteze i ponovo pravi MP3 samo ako svi opisi i dalje staju u sačuvane vremenske intervale. Intervali bez dijaloga, Visual Evidence vremena, Gemini opisi i vremenska analiza projekta ponovo se koriste bez ponovnog pokretanja AI analize.

Verzija 0.9.4 – 2026-09-04

Аудио-дескрипција уз AI
1. Исправљен је проблем са Matroska/WebM видео записима који садрже неуобичајено велики унутрашњи почетни временски жиг и због којег је генерисање аудио-дескрипције могло да се заустави уз грешку „invalid Gemini chunk timeline“. Sonarpad сада нормализује трајање изворне датотеке и Gemini сегмената само када открије овај аномални образац временских жигова, док нормални видео записи и већ функционално понашање аудио-дескрипције остају непромењени.
2. Побољшан је ducking аудио-дескрипције ради природнијег микса. Оригинални звук се сада постепено утишава још пре почетка гласа и после описа се мекше враћа на нормалан ниво; када су описи веома близу један другом, ниво позадине остаје стабилан без сталног спуштања и подизања јачине звука.
3. Исправљен је проблем због којег је завршни извоз аудио-дескрипције могао да не успе код појединих AVI датотека и других извора са 5.1/вишеканалним звуком, ако се декодирање завршило непотпуним испреплетаним аудио оквиром. Sonarpad сада безбедно допуњује само тај последњи делимични оквир строго неопходним бројем тихих узорака; већ потпуни оквири и уобичајене моно, стерео и вишеканалне датотеке остају непромењени.


Verzija 0.9.3 – 2026-09-03

SAPI5 glasovi
1. Ispravljen je problem zbog kojeg neki lokalni SAPI5 glasovi nisu govorili kada je uključeno pomeranje kursora, tokom čitanja sa više glasova ili pri pravljenju audio-knjiga/MP3 fajlova. Sonarpad sada koristi pouzdanu SAPI5 sintezu u fajl na Windowsu koja i dalje može da se otkaže tokom sinteze, dok direktno čitanje ostaje nepromenjeno.
2. Исправљен је положај курсора током читања дијалога са више гласова. Ознаке гласа које Sonarpad аутоматски додаје за дијалоге сада се третирају само као метаподаци репродукције, а не као знакови у уређивачу, па курсор после F4 или F6 више не прескаче испред стварног текста. Читање једним гласом и изричито уписане <voice> ознаке у документима остају непромењени.

Аудио-дескрипција уз AI
1. Одмах после поља за Gemini API кључ додато је поље „Прикажи API кључ“. Подразумевано је искључено; када се укључи, цео кључ се привремено приказује како би могло да се провери да ли је налепљен у целости. При поновном отварању прозора кључ је поново сакривен.

Podkasti i Wikipedia
1. Posle čuvanja snimka podkasta, Sonarpad sada pita da li treba otvoriti fasciklu koja sadrži sačuvanu datoteku, kao što već radi posle čuvanja YouTube/streaming medija.
2. Kada se novi Wikipedia članak uveze u editor koji već sadrži tekst, sada se dodaje na kraj umesto na početak. Kursor se postavlja na početak upravo uvezenog članka.


Verzija 0.9.2 – 2026-09-02

AI audio-deskripcija
1. Ispravljen je problem zbog kog je AI audio-deskripcija mogla da ne uspe tokom završnog izvoza u MP3 kod video zapisa sa višekanalnim zvukom, na primer 5.1. Sonarpad sada automatski pretvara višekanalni zvuk u stereo samo kada je to potrebno za MP3 kodiranje, bez promena mono ili stereo izvoza.
2. Kada se pokrene AI audio-deskripcija za video sa više zvučnih zapisa, Sonarpad sada pre obrade traži izbor zapisa koji će se koristiti. Pristupačna kombinovana lista menja se strelicama; OK pokreće audio-deskripciju sa izabranim zapisom, dok Otkaži zatvara prozor audio-deskripcije i vraća fokus u Sonarpad editor.

YouTube i strimovanje
1. Ispravljen je problem zbog kog je pokretanje AI audio-deskripcije za video sa druge ili neke naredne stranice YouTube plejliste ili kanala moglo ponovo da otvori prozor za izbor na YouTube-u i oduzme fokus prozoru audio-deskripcije. Sonarpad sada pravilno zatvara birač bez vraćanja na prethodne stranice.
Verzija 0.9.1 – 2026-09-01

YouTube preuzimanja
• Ispravljen je problem zbog kog su prozori napretka YouTube/streaming preuzimanja mogli više puta da se vrate u prvi plan nakon prelaska na drugu aplikaciju pomoću Alt+Tab. Preuzimanja sada nastavljaju u pozadini bez preuzimanja fokusa.
• Poboljšana je pristupačnost prikaza napretka preuzimanja. Kada se vratite u prozor napretka, čitači ekrana mogu da pročitaju trenutno stanje i procenat. Kod plejlista Sonarpad takođe saopštava broj trenutne stavke, ukupan broj stavki i naslov.
• Ispravljene su lažne watchdog prijave zamrzavanja tokom dugih preuzimanja i konverzija kada je prozor napretka i dalje reagovao.
• U preuzimanje plejlista dodat je kombinovani okvir Format. Iz liste video zapisa pritisnite Tab da izaberete MP4, MP3, M4A, OPUS, OGG, WAV ili FLAC pre pokretanja višestrukog preuzimanja.
• Preuređeno je čuvanje streaming medija. Format i kvalitet se sada biraju prilikom čuvanja, a ne u početnom prozoru za pretragu streaminga. „Sačuvaj medij“ otvara jedan dijalog za Format i Kvalitet, a preuzimanje plejlista sadrži oba kombinovana okvira.

AI audio-deskripcija
• Ispravljen je problem zbog kog AI audio-deskripcija nije mogla da se pokrene sa nekim MKV video-datotekama. Sonarpad sada pouzdanije obrađuje video sa nepravilnim ili nedostajućim vremenskim oznakama.

Verzija 0.9.0 – 2026-08-31

AI audio-deskripcija — velika nova funkcija
• U Alati > Multimedija dodata je stavka „Kreiraj audio-deskripciju pomoću AI“. Sonarpad analizira zvuk da pronađe mesta bez dijaloga, generiše opise pomoću Gemini-ja i koristi govorne motore koji su već dostupni u Sonarpadu, izbegavajući govor preko dijaloga.
• Poboljšana je sinhronizacija između onoga što se dešava u videu i generisanih opisa, uz automatske provere vremenskih oznaka koje daje Gemini.
• „Omogući produžene pauze“ je podrazumevano isključeno. Može se uključiti za sadržaj sa mnogo dijaloga ili malo slobodnog prostora kako bi se ipak ubacili duži opisi.
• Sonarpad može da pokuša da prepozna likove i koristi njihova imena. Katalozi likova mogu da se čuvaju između epizoda serije radi bolje doslednosti.
• Projekti mogu da se sačuvaju, kasnije uređuju i ponovo izvoze bez ponovnog generisanja svega pomoću Gemini-ja.
• Ako se proces prekine, Sonarpad čuva napredak i može da nastavi audio-deskripciju. Ako se potroši Gemini kvota, možete sačekati, promeniti model ili zaustaviti proces bez gubitka već završenog rada.
• Prozor omogućava izbor jezika, nivoa detalja, Gemini modela, govornog motora i glasa i pamti izabrane postavke.
• Modul je dostupan na svih 17 jezika Sonarpada. Tokom generisanja interfejs prikazuje samo napredak, trenutno stanje i Otkaži; po završetku MP3 može direktno da se otvori u internom plejeru.

E-knjige i dokumenti
• Dodat je uvoz Kindle datoteka bez DRM-a u formatima MOBI, AZW i AZW3, sa tekstom i poglavljima dostupnim u uređivaču i indeksu dokumenta.
• Dodata je podrška za DAISY 2.02 i DAISY 3. DAISY audio-knjige koriste interni Sonarpad plejer i poštuju navigaciju po poglavljima i granice reprodukcije.
• Kindle i DAISY datoteke uvoze se bez prepisivanja originala; Kindle knjige zaštićene DRM-om izričito se odbijaju.
• Ispravljeno je EPUB „Sačuvaj kao“: kada se izabere TXT ili drugi format, sada se koristi izabrana ekstenzija, a originalni EPUB ostaje povezan sa otvorenim dokumentom.

RSS i članci
• Dodat je višestruki izbor RSS članaka kako bi više članaka moglo da se obriše jednom radnjom.
• RSS sada podržava prave fascikle koje se čuvaju tokom OPML uvoza i izvoza, uključujući prazne fascikle.
• Izvori mogu da se preuređuju unutar trenutne fascikle komandama Pomeri gore, Pomeri dole, Pomeri na vrh, Pomeri na dno i Pomeri na poziciju.

Pristupačnost, vodiči i interfejs
• Sonarpad vodiči su reorganizovani i dobijaju indeks, a dodat je i kompletan vodič za AI audio-deskripciju.
• Ispravljen je problem u nemačkom prevodu koji je mogao da spreči pojavljivanje dijaloga Otvori, Sačuvaj kao i drugih dijaloga za izbor datoteka.

Glasovi i jezici
• Katalog Google TTS glasova za preuzimanje povećan je sa 104 na 156 paketa i sa 53 na 81 jezičku varijantu.
• Dodati su novi Google TTS paketi i lokalizovani nazivi za dodatne jezike širom interfejsa.

Verzija 0.8.4 – 2026-07-24

Uređivanje EPUB dokumenata
• Sonarpad sada može ne samo da otvara EPUB dokumente već i da ih uređuje i ponovo čuva u EPUB formatu uz očuvanje izvornog formatiranja, sadržaja, fusnota, slika, stilova, metapodataka i internih veza.
• EPUB je dostupan u „Sačuvaj kao“ za dokumente otvorene iz EPUB-a. Čuvanje ažurira samo izmenjeni tekst i ostavlja strukturu knjige netaknutom.

Pouzdanost audio-knjiga
• Ispravljen je povremeni problem gde je nakon pet neuspelih Google TTS pokušaja jedinica sinteze tiho odbacivana, pa je u konačnoj audio-knjizi mogao da nedostaje deo teksta.
• Google jedinice sada se ponavljaju dok ne uspeju ili korisnik ne otkaže. Pokretanje radnih procesa je raspoređeno kako bi se smanjili privremeni sukobi sa Chrome-om i datotekama, a Sonarpad sada zaustavlja proces umesto da sačuva audio-knjigu kojoj nedostaje segment.
• Edge audio-knjige sada ponavljaju privremene mrežne, WebSocket, timeout, ograničenje-usluge i nevažeći-audio odgovore dok ne uspeju ili korisnik ne otkaže, uključujući mešane glasove i podelu po vremenu. SAPI4 i SAPI5 zadržavaju adaptivni ograničeni oporavak; ako segment i dalje ne uspe, Sonarpad staje bez čuvanja nepotpune audio-knjige.

Navigacija u digitalnim bibliotekama
• Rezultati pretrage LibriVox-a, Internet Archive-a i Project Gutenberg-a sada koriste navigaciju po stranicama kao YouTube: „Idi na prethodne rezultate“ pojavljuje se na vrhu, a „Idi na sledeće rezultate“ na dnu.
• Ispravljeni su prelazi fokusa u LibriVox-u: otvaranje knjige ili poglavlja više ne šalje NVDA fokus u glavni uređivač pre nego što se otvori sledeća lista ili plejer.
• Dodat je čuvar fokusa za LibriVox tokom pretrage i učitavanja knjige: lokalizovani dijalog učitavanja ostaje u prvom planu dok zahtev traje i sprečava da NVDA fokus ode u Command Prompt, Windows Terminal ili drugu aplikaciju.

Preuzimanje YouTube plejlista
• YouTube plejlistama je dodata pristupačna komanda za višestruki izbor, koja omogućava korisnicima da izaberu koje video-zapise žele da preuzmu bez menjanja postojeće komande „Sačuvaj medij“ za trenutno puštenu stavku.
• Izabrane stavke preuzimaju se jedna po jedna u formatu i kvalitetu izabranom pri otvaranju plejliste, dobijaju numerisana imena datoteka koja čuvaju redosled plejliste i čuvaju se u posebnoj fascikli unutar podešene Media fascikle.
• Prozor za izbor sadrži komande Izaberi sve i Poništi izbor svega, saopštava broj izabranih stavki, podržava otkazivanje uz zadržavanje završenih datoteka i prijavljuje stavke koje nisu mogle da se preuzmu.
• Stavke plejliste sada su standardna polja za potvrdu: čitači ekrana automatski saopštavaju svaki naslov, ulogu kontrole i stanje označenosti, bez dodavanja reči o izboru vidljivom naslovu ili prinudnog govora.

Verzija 0.8.3 – 2026-07-23

Tamni režim
• Dodat je tamni režim koji se može uključiti iz menija Prikaz i čuva se u korisničkim podešavanjima.
• Tamna tema se primenjuje na uređivač, menije, sekundarne prozore i glavne kontrole, uz prilagođene boje teksta radi očuvanja čitljivosti i pristupačnosti.

Nemački jezik
• Nemački je dodat kao kompletan jezik interfejsa i može se izabrati u Opcijama.
• Vesti i RSS, provera pravopisa, kalendar i svi citati, donacije, vodič i dnevnik izmena potpuno su dostupni na nemačkom.

Brazilski portugalski i Google vesti
• Brazilski portugalski dodat je kao kompletan jezik interfejsa, odvojen od portugalskog (Portugal), i može se izabrati u Opcijama.
• Kompletan interfejs, kalendarske stavke i citati, provera pravopisa, donacije, vodič i dnevnik izmena dostupni su na brazilskom portugalskom.
• Google vesti sada podržavaju brazilsku lokalizaciju, brazilske kategorije i posebne podrazumevane brazilske RSS izvore.
• Povezani Google vesti izvori za istu priču prikazuju se kao pristupačne podstavke u stablu kada ih izvor pruža.

LibriVox
• Optimizovane su LibriVox pretrage kako bi se izbegao prevelik broj zahteva prema servisu i zamrzavanje interfejsa. Uklonjeno je veliko skeniranje kataloga, smanjen broj pokušaja i uvedeni kraći vremenski limiti.

Sinteza govora
• Nizovi od tri ili više tačaka sada se normalizuju pre čitanja, sprečavajući neke glasove da izgovaraju „tačka tačka“ ili da generišu segmente sastavljene samo od interpunkcije.

Povezani članci Google vesti
• Za svaku vest sada se prikazuju povezani članci kada postoje, odnosno drugi članci koji obrađuju istu priču. Da biste ih pročitali, jednostavno proširite glavni članak kada Sonarpad saopšti da postoje povezani članci. Korisnici koji ne žele da prošire ovaj deo mogu samo pritisnuti Enter na glavnom članku i pročitati vest kao i obično.
• Povezani članci sada koriste isti sistem pročitano/nepročitano kao glavni članci, uključujući pristupačna obaveštenja, datum i vreme, stanje sačuvanosti i očuvanje nakon ažuriranja izvora ili ponovnog pokretanja Sonarpada.

Najave delova audio-knjige
• U Audio opcije dodato je kombinovano polje „Najava na početku svakog dela“. Za audio-knjige podeljene u više datoteka svaki deo može početi bez najave, naslovom knjige, naslovom i brojem dela, imenom datoteke ili imenom datoteke i brojem dela.

Verzija 0.8.2 – 2026-07-17

Digitalne biblioteke i audio-knjige
• Dodat je Project Gutenberg, sa pretragom po naslovu ili autoru i izborom jezika.
• Project Gutenberg EPUB knjige preuzimaju se u Documents\Sonarpad\Documents; kada se preuzimanje završi, Sonarpad pita da li knjigu odmah otvoriti u uređivaču.
• Dodat je Internet Archive za pretragu i slušanje audio kolekcija, uključujući stare radio-emisije, govore i živu muziku.
• Dodat je LibriVox za pretragu audio-knjiga po naslovu ili autoru i direktno puštanje poglavlja istim plejerom koji se koristi za podkaste.
• Sve tri nove funkcije dostupne su u meniju Alati i, kada je grupisanje menija uključeno, u odeljku Čitanje.

Transkripcija dugog zvuka
• Ispravljena je transkripcija dugih audio datoteka: zvuk se sada automatski deli na delove od 15 minuta, transkribuje deo po deo, a zatim ponovo spaja, čime se sprečavaju greške koje su mogle da se jave kod dugih snimaka.

YouTube
• Najkorisnije radnje koje su ranije bile dostupne samo nakon otvaranja YouTube videa i menija Reprodukcija sada su dostupne i direktno iz kontekstnog menija tog videa, kao što su „Transkribuj trenutni zvuk“, „Kreiraj audio-deskripciju pomoću AI“ i „Sačuvaj medij“.
• Dodata je komanda „Kopiraj vezu“, dostupna i sa Ctrl+C, za kopiranje URL-a izabranog YouTube videa, plejliste ili kanala u ostavu.

Verzija 0.8.1 – 2026-07-16

Google pretvaranje teksta u govor
• Ispravljeno je pokretanje Google TTS-a na Windows sistemima gde su veze koje prihvata interni server pregledača nasleđivale neblokirajući režim soketa, što je izazivalo grešku 10035 i sprečavalo preuzete glasove da govore.
• Sonarpad sada čeka da se Chrome ili Edge WASM motor potpuno učita pre pregleda glasa ili čitanja pomoću F5, čime se sprečava greška „Chrome WASM TTS engine was not loaded“.
• Skriveni pregledač isključuje prevođenje stranica i pristupačnost renderera, pa više ne može da saopšti „Prevedi stranicu“ niti da ometa komande čitanja.
• Panel „Glasovi u uređivaču“ sada prikazuje dugme „Upravljaj Google glasovima...“ kad god je izabran Google motor i odmah osvežava listu instaliranih glasova po zatvaranju upravljača.
• Upozorenja o zavisnostima prikazana pri uklanjanju Google paketa glasova sada su lokalizovana na svakom jeziku interfejsa.

Iskustvo ažuriranja
• Posle automatskog ažuriranja, prozor završetka i dnevnika izmena sada se otvara nakon početnog vraćanja fokusa u uređivač i ostaje u prvom planu umesto da se pojavi tek nakon pritiska na Tab.

PDF dokumenti
• Ispravljene su PDF datoteke čiji je ugrađeni tekst sadržao NUL znakove i bio odsečen na prvom takvom znaku pri učitavanju u uređivač.
• Kada pdf-extract vrati ugrađene NUL znakove, Sonarpad sada pokušava ponovo pomoću PDFium-a; preostali NUL znakovi uklanjaju se pre slanja teksta Windows kontrolama, pa se ostatak dokumenta čuva.

Pristupačnost menija
• Uklonjeno je generisanje mnemonika tokom rada: pristupni tasteri sada su izričito upisani u svih 15 prevoda interfejsa i zato ostaju isti između pokretanja.
• Pregledane su sve stabilne stavke i podmeniji glavnog menija, uključujući Reprodukciju, izbore fonta, Sačuvaj sliku i Prikaži EPUB indeks; nedostajući ili duplirani mnemonici među susednim stavkama ispravljeni su direktno u prevodima.
• Automatski testovi sada samo proveravaju prevode i padaju ako mnemonic nedostaje, nije ispravan ili je dupliran; nikada ne menjaju oznake menija tokom rada.
• U izuzetno velikim menijima gde prevedene oznake ne nude dovoljno različitih znakova prikazuje se eksplicitan numerički pristupni taster u standardnom Windows obliku „(&1)“.

Verzija 0.8.0 – 2026-07-15

Onlajn rečnik
• Dodat je nemački u onlajn Wiktionary rečnik.
• Nemačke definicije i sinonimi sada se analiziraju prema strukturi nemačkog Wiktionary-ja, umesto da se jezik samo doda na listu izbora.

Pouzdanost SAPI5 audio-knjiga
• Kreiranje SAPI5 audio-knjiga zadržava do 12 paralelnih radnih procesa kada izabrani glas daje pouzdan rezultat.
• Svaki generisani deo sada se proverava prema veličini datoteke, procenjenom trajanju i konzervativnom poređenju sa dodeljenim tekstom.
• Nedostajući ili sumnjivi delovi automatski se ponovo generišu sa postepeno manjom konkurentnošću: 12, 8, 6, 4, 2 i na kraju 1 radni proces. Ponavljaju se samo problematični delovi.
• Pouzdana granica broja radnih procesa pamti se posebno za svaki SAPI5 glas, bez usporavanja glasova koji pravilno rade sa 12 procesa.
• Završna provera integriteta sprečava Sonarpad da tiho prihvati MP3 koji je mnogo kraći od generisanih delova.
• Detaljna dijagnostika se upisuje u `sapi5_audiobook_diagnostic.log`.
• Svaka SAPI5 jedinica sinteze sada radi u zasebnom skrivenom Sonarpad procesu. Ako glas treće strane padne, zatvara se samo taj radni proces, a glavna aplikacija ostaje otvorena.
• Tokom istog kreiranja audio-knjige nezavršeni delovi odmah se ponavljaju sa sledećim nižim nivoom konkurentnosti; već provereni delovi se čuvaju.
• Oporavak pri sledećem pokretanju ostaje kao dodatna zaštita samo ako se prekine glavna aplikacija ili računar.

SAPI4 radni procesi za audio-knjige
• Sada se poštuje broj SAPI4 procesa koji je korisnik izabrao, do tehničkog maksimuma od 64; prethodno skriveno ograničenje od 16 je uklonjeno.
• Efektivni broj se smanjuje samo kada audio-knjiga sadrži manje radnih jedinica nego što je traženo.
• Ako jedan ili više SAPI4 bridge procesa ne uspe, završeni delovi se čuvaju, a samo neuspele jedinice automatski se ponavljaju sa postepeno manjom konkurentnošću.
• Sonarpad sada proverava izlazni status SAPI4 bridge-a i odbacuje prazne ili nevažeće audio-delove umesto da ih smatra uspešnim.

Podešavanje proxy-ja
• Dodato je posebno polje za proxy port u mrežnim podešavanjima.
• Port sada može da se unese nezavisno od proxy adrese, proverava se u opsegu od 1 do 65535 i pravilno zamenjuje port koji je već naveden u URL-u.

Pretraga radija po jeziku i zemlji
• Filteri Jezik i Zemlja sada se ažuriraju svim dostupnim stavkama iz Radio Browser direktorijuma umesto da budu ograničeni na fiksnu listu.
• Nazivi jezika sada se prepoznaju čak i kada ih Radio Browser isporuči drugim pismom, kao izvorne nazive, skraćenice ili kombinacije više jezika, i prikazuju se prevedeni na trenutni jezik interfejsa. Vrednosti koje nisu stvarni jezici, kao što su brojevi, žanrovi, zemlje ili opšte oznake, filtriraju se.
• Direktorijum se osvežava u pozadini, uz rezervnu listu koja ostaje upotrebljiva kada Radio Browser nije dostupan.
• Duplikati jezičkih stavki Radio Browser-a koje nakon prevoda postanu iste sada se spajaju u jednu stavku kombinovanog polja, sprečavajući tihe korake sa čitačima ekrana.

Veliko poboljšanje: sinhronizacija govora i kretanja kursora
• Sinhronizacija između reprodukcije govora i kretanja kursora značajno je poboljšana za svaki podržani govorni motor.
• Kada je uključeno „Pomeraj kursor tokom čitanja“, Sonarpad sada koristi zajednički sistem napretka za Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 i OneCore.
• Kursor preciznije prati tekst koji se zaista izgovara, uz doslednije segmentiranje rečenica i fraza.
• Prerano pomeranje, kašnjenja, nepravilni skokovi i razlike između govornih motora znatno su smanjeni.
• Ispravan položaj se sada pouzdanije čuva nakon pauze, nastavka, pretrage u dokumentu ili promene govornog motora.

Odvojene trake za snimanje podkasta
• Dodato je „Sačuvaj mikrofon i sistemski ili zvuk aplikacije u odvojene datoteke“.
• Kada se mikrofon i drugi izvor snimaju zajedno, Sonarpad može da napravi jednu datoteku samo sa mikrofonom i drugu sa sistemskim zvukom, jednom aplikacijom ili izabranim aplikacijama.
• Odvojeno snimanje izvora dostupno je i u MP3 i u WAV formatu.
• Kada je opcija isključena, Sonarpad nastavlja da pravi jednu normalno miksovanu datoteku.
• Odvojene datoteke olakšavaju podešavanje jačine zvuka, uklanjanje šuma i kasnije uređivanje podkasta, intervjua i tutorijala.

Zakazana radio-snimanja
• Radio-snimanja sada mogu unapred da se zakažu.
• Za svako snimanje korisnik može da izabere stanicu, dan, početni sat i minut i trajanje.
• Dostupno je prilagođeno trajanje od 1 do 1.440 minuta.
• Snimanja mogu da se pokrenu jednom, svakog dana ili svake nedelje.
• Prozor snimanja sada jasnije prikazuje aktivna i zakazana snimanja, planirani datum i vreme, trajanje i preostalo vreme do početka.
• Zakazana snimanja mogu da koriste Windows Task Scheduler, što im omogućava da se automatski pokrenu čak i kada Sonarpad nije već otvoren.

Kalendar
• Dodat je kompletan kalendar dostupan sa tastature.
• Korisnici mogu da pregledaju prethodne i naredne dane, brzo se vrate na danas i provere praznike i obeležavanja.
• Dodat je svetac dana i citat dana, koji mogu da se pročitaju, izgovore ili kopiraju.
• Podsetnici mogu da se kreiraju, uređuju, brišu, odlažu i označe kao završeni.
• Upozorenja mogu da se prikažu u tačno vreme ili unapred i mogu da koriste Windows raspoređivanje čak i kada je Sonarpad zatvoren.

Vreme
• Dodat je odeljak vremenske prognoze.
• Korisnici mogu da pretraže grad i brzo ponovo otvore nedavno pregledane lokacije.
• Dostupni su trenutni uslovi, temperatura, minimalne i maksimalne vrednosti, vlažnost, verovatnoća padavina i prognoze za naredne dane.
• Temperatura može da se prikaže u Celzijusima, Farenhajtima ili da se izabere automatski.

Filmovi u bioskopima
• Dodat je odeljak za filmove koji su trenutno u bioskopima i predstojeća izdanja.
• Dostupni su pretraga po naslovu, radnja, datum izlaska i reprodukcija trejlera.

Google pretvaranje teksta u govor
• Dodat je Google TTS za čitanje dokumenata i kreiranje audio-knjiga.
• Dodat je upravljač glasovima za prikaz glasova, filtriranje po jeziku, preuzimanje i uklanjanje glasova koji više nisu potrebni.
• Brzina, jačina i visina glasa mogu da se podešavaju.
• Visina Google Natural glasa primenjuje se direktno u motoru radi prirodnijeg i stabilnijeg rezultata.
• Poboljšani su odziv i pouzdanost Google TTS-a, sa vremenskim granicama sinteze prilagođenim izabranoj brzini govora.
• Smanjeno je nepotrebno čekanje kada motor ne odgovara, a poboljšano je rukovanje greškama i prekidima.
• Dijagnostičko beleženje je stabilnije tokom istovremenih operacija.

EPUB sadržaj
• Sonarpad sada prepoznaje sadržaj ugrađen u EPUB knjige.
• Njegovo prisustvo se saopštava i može da se otvori iz menija Prikaz.
• Poglavlja i potpoglavlja prikazuju se hijerarhijski.
• Pritiskom na Enter odmah se prelazi na izabrano mesto u knjizi.

Izvori vesti i RSS-a
• Odeljak Vesti proširen je novim alatima za pretragu i organizaciju.
• Dodat je izbor jezika vesti.
• Korisnici mogu da pretražuju RSS izvore i čitaju vesti iz svog grada.
• RSS izvori zajednice mogu da se pregledaju, dodaju u ličnu kolekciju i pošalju Sonarpad zajednici.

Snimanje podkasta
• Korisnici mogu da snimaju samo mikrofon, sav sistemski zvuk, jednu aplikaciju, više izabranih aplikacija ili mikrofon i aplikacije zajedno.
• Mogu se izabrati mikrofonski uređaj i izvor zvuka, jačine izvora podešavati odvojeno i nivoi pratiti u realnom vremenu.
• Dodati su pauza i nastavak, MP3 ili WAV izlaz, izbor MP3 bitrate-a i izbor odredišne fascikle.
• Računar može da ostane budan tokom snimanja.
• Odvojene datoteke dobijaju različita imena kako bi se mikrofonska traka odmah razlikovala od sistemskog ili zvuka aplikacije.

Radio
• Odeljak Radio je opsežno reorganizovan.
• Stanice mogu da se pretražuju po imenu ili slobodnom tekstu, jeziku, zemlji, gradu, muzičkom žanru ili kategoriji.
• Poboljšano je upravljanje omiljenim stanicama i svi filteri mogu brzo da se ponište.
• Stanice mogu da se pošalju Sonarpad zajednici.
• Dodati su snimanje uživo, „Snimaj i pusti“, lista snimaka i brisanje i upravljanje snimcima.
• Radio-snimci čuvaju se u sopstvenoj fascikli unutar glavnog direktorijuma snimaka.

Reprodukcija medija
• Značajno je poboljšana stabilnost medijskog plejera.
• Ispravljen je problem koji je mogao da blokira mpv i poboljšana je pouzdanost komunikacije sa plejerom.
• Poboljšano je otvaranje različitih tipova medijskih datoteka.
• Sonarpad sada pamti jačinu korišćenu tokom reprodukcije.
• Poboljšano je rukovanje strimovima i snimcima.
• Ispravljene su datoteke otvorene iz Windows-a dvoklikom ili preko „Otvori pomoću“.

PDF dokumenti
• Dodato je prepoznavanje polja obrazaca u PDF dokumentima.
• Sonarpad može da pronađe polja koja se popunjavaju, predstavi ih u pristupačnom tekstualnom obliku, omogući uređivanje vrednosti i sačuva unete podatke nazad u PDF.
• Ispravljeno je računanje položaja kursora tokom govora, posebno u dokumentima sa višebajtnim znacima ili složenim strukturama.
• Novi zajednički sistem sinhronizacije dodatno poboljšava kretanje kursora sa svakim govornim motorom.

Pristupačnost i komande sa tastature
• Poboljšane su standardne komande za uređivanje širom programa.
• Kopiranje, isecanje, lepljenje, izbor svega, poništavanje i ponavljanje sada se pravilno šalju polju koje ima fokus, uključujući sekundarne prozore i dijaloge.
• Ispravljen je problem koji je mogao da spreči pravilno osvežavanje Brajevih redova.
• Poboljšano je rukovanje fokusom u sekundarnim prozorima.
• Ispravljen je izbor jezika u Wikipedia prozoru.
• Dodata je opcija da se funkcije menija Alati grupišu po kategorijama.
• Dodate su podesive radnje za brzo otvaranje Kalendara, Vremena i Filmova u bioskopima.
• Poboljšan je prikaz dnevnika izmena nakon ažuriranja.

Audio-knjige
• Poboljšano je kreiranje audio-knjiga dok su otvoreni dijalozi ili drugi modalni prozori.
• Rukovanje napretkom je robusnije i ignoriše zastarela audio-ažuriranja, čime se smanjuju zamrzavanja, pogrešna obaveštenja i prozori koji ne odgovaraju.
• Google TTS može da se koristi i za kreiranje audio-knjiga uz kontrolu brzine, jačine i visine glasa.

Veštačka inteligencija
• Podrazumevani Gemini model ažuriran je na `gemini-3.5-flash`.

Opšte ispravke
• Ispravljeno je nekoliko zamrzavanja mpv reprodukcije.
• Ispravljeno je otvaranje nekih audio i video datoteka.
• Poboljšane su komande koje se šalju medijskom plejeru.
• Ispravljeno je vraćanje kursora tokom reprodukcije govora.
• Ispravljene su prečice u tekstualnim poljima pomoćnih prozora.
• Poboljšana je stabilnost kreiranja audio-knjiga.
• Ispravljene su datoteke otvorene spolja preko Windows-a.
• Poboljšano je ukupno rukovanje medijima, RSS-om, radiom i EPUB-om.

Verzija 0.7.1 – 2026-05-13

Nove funkcije i poboljšanja
• Napravljen je zvanični sajt sonarpad.com, novo mesto za najnovije vesti, preuzimanje najnovije verzije, komentare posetilaca i, ubuduće, sve Sonarpad podkaste. Meni Pomoć sada sadrži i „Poseti sonarpad.com“.
• Ispravljen je problem gde su datoteke sa akcentima ili posebnim znakovima izazivale grešku pri pokretanju glasovne transkripcije.
• Stavke kao što su Prelamanje redova i Prikaži video tokom reprodukcije u meniju Prikaz sada uvek pokazuju tačno stanje, uključeno ili isključeno.
• Poboljšana je YouTube pretraga tako da korisnici mogu da se vrate na prethodnu stranicu ili ekran tasterom Esc.
• Dodata je preliminarna provera da li video može da se reprodukuje. Sonarpad sada može da reprodukuje i videe ili plejliste označene kao miksevi, što ranije nije bilo moguće.
• Poboljšano je automatsko upravljanje obeleživačima. Ranije su automatski obeleživači ostajali nakon isključivanja opcije; sada ih program pravilno ignoriše dok se opcija ponovo ne uključi. Na kraju medijske datoteke obeleživač se automatski briše.
• Poboljšano je rukovanje oznakama kada su dijalozi uključeni. Sonarpad sada pravilno upravlja obe funkcije i omogućava umetanje oznaka čak i kad je uključena opcija dijaloga.
• Poboljšana su podešavanja glasova jasnim razdvajanjem svakog motora. Glasovni profili sada pravilno čuvaju postavke za svaki motor: Edge, SAPI5 i SAPI4.
• Dodata je oznaka za umetanje pauza, direktno iz opcija ili glasovnog panela pritiskom na Tab iz uređivača. Mogući izbori su 250 ms, 500 ms, 1 sekunda, 2 sekunde ili prilagođeno trajanje.
• Ispravljeno je ponašanje pri reprodukciji YouTube videa i pokretanju transkripcije. Po povratku Alt+Tab-om fokus je sada pravilno na dugmetu Otkaži aktivne transkripcije.
• Transkripcije se sada automatski čuvaju kada se proces završi.
• Poboljšan je Wikipedia uvoz. Možete da čitate samo jedan odeljak pa se iz članka vratite u pretragu pomoću Esc, ili da uvezete ceo članak. Može se izabrati i Wikipedia jezik.
• Dodat je svetski radio odeljak sa pretragom stanica po zemlji, jeziku i žanru. Lokalne stanice mogu da se dodaju u Sonarpad bazu da bi ih slušali i drugi korisnici, a mogu se dodati i u omiljene.
• Dodat je odeljak za rute sa načinima putovanja: pešice, biciklom, automobilom ili invalidskim kolicima. Može se izabrati najkraća ili najbrža ruta i prikaz opština kroz koje prolazi. Uvezena ruta može da se sačuva i kao vizuelna mapa kroz Datoteka > Sačuvaj sliku.
• U meni Datoteka dodata je Štampa. Sonarpad TXT datoteke štampa sopstvenim sistemom, a za DOCX, PDF i slične formate koristi pridruženi program kako bi što više sačuvao originalni raspored.
• Dodata je usluga prevođenja za svaki dokument, dostupna iz kontekstnog menija uređivača. Besplatni DeepL i Google Translate mogu se koristiti bez API ključa; unosom Gemini API ključa može se prevoditi pomoću Gemini-ja.
• U meniju prevođenja može se izabrati ciljni jezik. Meni se automatski preuređuje: ako se prvo izabere engleski, zatim francuski pa italijanski, te tri opcije se prikazuju na vrhu menija jezika.
• Ako korisnik unese Gemini API ključ, dobija i funkciju Sažmi tekst iz kontekstnog menija za sažimanje bilo kog članka.
• U meni Reprodukcija dodat je meni, vidljiv tokom reprodukcije medija, za deljenje trenutnog medija. Radi sa MP3, MP4 i drugim formatima i deli po broju delova ili trajanju svakog dela.

Verzija 0.7.0 – 2026-04-25

Šta je novo
• Dodata je podrška za mpv plejer pri striming reprodukciji. Video sa YouTube-a i podržanih sajtova sada se pušta odmah; ako korisnik želi da ga zadrži, preuzima se kao ranije. Pri transkripciji striming sadržaja on se prvo preuzima pa transkribuje. mpv se koristi i za lokalne videe i titlove, sa boljom kompatibilnošću sa mnogim formatima koji ranije nisu bili potpuno podržani.
• Poboljšano je snimanje sistemskog zvuka za podkaste: sada se može izabrati sav sistemski zvuk, jedna aplikacija ili više aplikacija istovremeno. Mikrofon se i dalje može zasebno uključiti ili isključiti.
• Dodat je hindi jezik. Preveden je interfejs i dodati RSS izvori, dnevnik izmena i Sonarpad vodič.
• U kartici Uređivač dodata je opcija da strelice gore/dole uvek pomeraju kursor na početak reda.
• U meni „Konvertuj audio“ dodat je M4B.

Ispravke
• Ispravljen je `F10` da ponovo prelazi na sledeći omiljeni glas tokom čitanja teksta.
• Dok je snimanje podkasta aktivno, zatvaranje drugog dokumenta više ne zatvara aktivno snimanje.
• U YouTube komentarima otvorenim iz „Pusti striming audio...“, Sonarpad prvo učitava samo prvih 50 komentara najvišeg nivoa, uvek sa svim odgovorima, i dodaje poslednju stavku za učitavanje svih komentara po potrebi.
• Obeleživači se sada prikazuju i obrađuju po položaju i za tekst i za medije, umesto po redosledu kreiranja. Obeleživač na istoj poziciji više se ne dodaje ponovo.
• U meni Obeleživači dodata je opcija za automatsko upravljanje. Kada se lokalna ili striming datoteka reprodukuje i zatvori, Sonarpad automatski čuva dostignutu poziciju i nastavlja odatle pri sledećem otvaranju. Isto važi za tekst: pamti se položaj kursora, a pri čitanju se čuva poslednja pročitana rečenica.
• U meni Prikaz dodata je stavka za prikaz videa za lokalne ili striming datoteke. Video se prikazuje u uvećanom prozoru sa skrivenim kontrolama dok se ne pritisne Alt ili miš pomeri ka vrhu, radi bolje upotrebljivosti za slabovide korisnike.

Verzija 0.6.9 – 2026-04-08

Ispravke
• Poboljšana je Pretraga u datotekama: pri Otvori fasciklu fokus odmah ide na listu fascikli; Enter na rezultatu više ne kvari komande; Esc vraća prethodno izabrani rezultat; a nakon Alt+Tab fokus ide na polje pretrage ili listu rezultata ako je otvorena.
• F5 je uvek počinjao čitanje od početka. Sada čitanje kreće od trenutnog položaja kursora, uz zadržavanje `Shift+F5` i `Ctrl+F5` za prethodnu i sledeću rečenicu.
• Posle Idi na red, Esc je mogao da izbaci fokus iz Sonarpada. Sada se fokus pravilno vraća u uređivač.
• Opcija `Prelamanje redova` sada se odmah primenjuje na već otvorene dokumente, umesto tek nakon ponovnog otvaranja.

Verzija 0.6.8 – 2026-04-07

Šta je novo
• U meni Reprodukcija dodata je nova stavka za transkripciju bilo koje audio ili video datoteke pomoću Whisper-a. U Opcijama je dostupan novi odeljak „AI i transkripcija“, gde se bira model, opcioni CUDA za NVIDIA grafičke kartice, čuvanje originalnog jezika i uključivanje ili isključivanje vremenskih oznaka.
• Dodata je nova stavka u meniju Reprodukcija, `Transkribuj trenutnu fasciklu`, koja transkribuje sve podržane audio datoteke iz fascikle trenutno otvorenog medija u jedan objedinjeni dokument, sa posebnim prikazom napretka, statusom trenutne datoteke i mogućnošću otkazivanja. Može se pokrenuti i prečicom `Alt+Shift+C`.
• Dodato je oflajn glasovno diktiranje koje koristi isti tok kao audio transkripcija. Podrazumevano pritisnite `Ctrl+Shift+Space` za početak diktiranja i ponovo istu prečicu za zaustavljanje; prečica se može prilagoditi u Opcijama. Od drugog pokretanja nadalje diktiranje je brže jer motor ostaje spreman u memoriji; ovo prethodno učitavanje i ponovno korišćenje automatski se isključuju na računarima sa manje od 4 GB RAM-a.
• U Editor je dodata nova opcija, podrazumevano isključena, koja omogućava da `Esc` zatvori prozor editora.
• Pretraga podkasta sada podrazumevano koristi `iTunes + Spreaker`, uz filtriranje duplikata kada se isti podkast pronađe na obe platforme.
• Poboljšano je pregledanje i pretraživanje Apple podkasta: pretraga, pregled kategorija i najpopularniji podkasti po kategoriji sada koriste izabranu zemlju direktorijuma podkasta. U Opcije > RSS / Podkast možete ostaviti `Automatski` da se koristi zemlja sistema ili ručno izabrati drugu zemlju.
• Povećano je ograničenje rezultata za Apple kategorije podkasta. Pri prvom otvaranju i dalje se učitava prvih 50 rezultata; ako izaberete `Učitaj još rezultata`, Sonarpad učitava do ukupno 200 rezultata, što je Apple ograničenje, i omogućava glatko kretanje kroz naredne stranice.
• Sonarpad je sada dostupan i na Mac-u sa podskupom funkcija. Link projekta: https://github.com/Ambro86/Sonarpad-Mac

Poboljšanja
• Dodato je više od 50 zemalja koje se mogu izabrati za direktorijum podkasta, pa korisnici mogu birati mnogo širi izbor nacionalnih kataloga.
• „Reprodukuj striming audio...” sada može i da pretražuje YouTube iz bilo kog tekstualnog upita ili da prihvati link YouTube kanala ili plejliste i prikaže rezultate.
• Poboljšan je prikaz rezultata u „Reprodukuj striming audio...”: YouTube stavke sada jasnije prikazuju naslov, trajanje, kanal i broj pregleda.
• „Reprodukuj striming audio...” sada podržava i YouTube komentare: mogu se otvoriti iz kontekstnog menija, čitati odgovori i proširivati niti komentara tasterom Strelica desno.
• Dodati su YouTube favoriti za kanale i plejliste u „Reprodukuj striming audio...”: mogu se dodati iz rezultata preko kontekstnog menija, otvoriti direktno sa liste Favoriti do koje se dolazi Tab-om odmah posle polja za YouTube URL/upit i kasnije ukloniti iz iste liste preko kontekstnog menija. U YouTube rezultatima pretrage kontekstni meni je dostupan samo za kanale i plejliste.
• „Reprodukuj striming audio...” sada može da zatraži podatke za prijavu kada striming sajt zahteva autentifikaciju. Korisnik može da ih unese, sačuva za taj sajt i kasnije upravlja sačuvanim podacima u Opcije > Audio.
• Poboljšano je upravljanje fokusom tokom „Reprodukuj striming audio...”, pa prozor napretka ostaje stabilniji tokom preuzimanja i konverzije.
• U meni Glas dodate su dve nove radnje za kretanje pri čitanju: `Prethodna rečenica` i `Sledeća rečenica`, sa prilagodljivim prečicama.
• Podrazumevana prečica za `Izvrši datoteku interpreterom` sada je `Ctrl+Shift+F5`, pa `Shift+F5` može podrazumevano da se koristi za `Prethodnu rečenicu`.
• U Opcije > Glas dodati su glasovni profili: profili se mogu dodavati, preimenovati i brisati.
• Proširene su opcije intervala vraćanja tokom reprodukcije u Opcije > Audio dodatnim vrednostima od 1 sekunde do 2 sata.
• Dodata je ruska lokalizacija, zahvaljujući Dmitriyu.
• U Opcije > Audio dodata je nova opcija za format naziva delova audio-knjige: `Naslov + broj`, `Samo broj` ili `Broj + naslov`.
• Dodati su omiljeni RSS članci: iz kontekstnog menija članka stavke se mogu dodati u poseban kanal Favoriti.
• RSS kanal Favoriti može se obrisati i automatski se ponovo kreira kada se novi članak doda u favorite.
• Dodate su RSS prečice za pomeranje kanala gore/dole: `Ctrl+Shift+Strelica gore` i `Ctrl+Shift+Strelica dole`.
• Poboljšan je RSS prozor ugrađenim pregledom članka, tako da se tekst članka može pregledati direktno u njemu i brzo dohvatiti Tab-om pre otvaranja celog članka u editoru.
• Na kraju RSS kanala dodata je jasna stavka „Učitaj još vesti” kada ima još sadržaja; Enter učitava sledeću grupu i premešta fokus na prvi novoučitani članak.
• U rečniku glasovnih zamena, pri dodavanju ili uređivanju zamene sada postoji polje „Razlikuj velika i mala slova”, tako da svaka zamena može poštovati ili zanemariti veličinu slova.
Ispravke grešaka
• „Reprodukuj striming audio...” sada poštuje ograničenje keša podkasta podešeno u Opcijama, a isto ograničenje sada važi i za reprodukciju audio-deskripcija.
• Ispravljen je uvoz sa Wikipedije tako da se blokovi citata sa stranica sada pravilno uvoze.
• Poboljšan je analizator veb-stranica za WordPress stranice na kojima su stavke lista i neki naslovi odeljaka mogli biti izostavljeni.
• „Idi na red” sada unapred popunjava polje trenutnim redom.
• Ispravljen je OPML izvoz podkasta i RSS-a tako da iTunes sada prihvata izvezene datoteke.
• Dodate su lokalizovane poruke potvrde za uspešan OPML uvoz i izvoz RSS kanala i podkasta.
• Ispravljena je greška u „Reprodukuj striming audio...” gde je unos upita i izbor YouTube kanala iz rezultata mogao da učini da program izgleda zaglavljeno umesto da otvori video-zapise tog kanala.
• Ispravljena je greška zbog koje se lista otvorenih datoteka prikazivala u meniju Pomoć umesto u meniju Prozor.
• Ispravljen je rubni slučaj striminga gde je reprodukcija mogla da počne, ali je dijalog „Preuzimanje strima” ostajao otvoren kada je preuzeta datoteka već odgovarala ciljnom formatu.
• Ispravljeno je ponašanje MP3 konverzije strima: kada je strim već MP3 i korisnik izabere konkretnu MP3 bit-brzinu, na primer 128 kbps, Sonarpad sada ponovo kodira na izabranu vrednost umesto da preskoči konverziju.
• Ispravljeni su dokumenti transkripcije medija tako da pri zatvaranju sada pitaju da li treba sačuvati, a predloženo ime datoteke pravilno koristi ime transkribovanog medija umesto prvog reda teksta.
• Ispravljena je prečica `Alt+Shift+L`: sada pravilno otvara listu poglavlja tokom reprodukcije.
• Ispravljena je prečica `Alt+Shift+T`: sada pravilno pokreće „Transkribuj trenutni audio” umesto da otvori meni Alati.
• Ispravljeno je zaustavljanje iz menija Reprodukcija: pritiskom na `.` sada se ponaša kao Stop i zaustavlja samo trenutnu numeru, bez izlaska iz plejera/epizode.
• Ispravljena je stavka za čuvanje u meniju Reprodukcija za medije otvorene iz Nedavnih datoteka: kada datoteka dolazi iz lokalnog Sonarpad keša, lokalizovana radnja čuvanja sada se pravilno prikazuje i tamo.
• Kada transkripcija počne dok se audio već reprodukuje, Sonarpad sada automatski pauzira audio pre početka transkripcije.
• Ispravljena je greška gde je uvoz članka sa Wikipedije mogao uspeti bez prikaza teksta članka na ekranu.
• Dodata je podrška za ugrađena poglavlja podkasta iz lokalnih medijskih datoteka, na primer MP3 metapodatke poglavlja: kada poglavlja iz feed-a/URL-a nisu dostupna, Sonarpad ih učitava iz preuzete datoteke u pozadini, pa reprodukcija počinje odmah, a podaci o poglavljima se primenjuju čim budu spremni.
• Ispravljeno je učitavanje poglavlja za preuzete epizode podkasta otvorene kao obične lokalne medijske datoteke: ugrađena poglavlja sada su dostupna i tamo, ne samo kada reprodukcija počne iz prozora Podkasti.
• Ispravljena je završna obrada MP3 audio-knjiga za SAPI4 i SAPI5: završna datoteka se sada pravilno finalizuje kako bi se izbegle nepotpune ili osetljive datoteke nakon dugih izvoza.
• Dodata je posebna traka napretka završne obrade za sve režime stvaranja audio-knjiga: posle faze kreiranja Sonarpad sada najavljuje i prikazuje posebnu fazu finalizacije sa vidljivim napretkom.
• Ispravljeno je podešavanje glasova za dijaloge: brzina, visina i jačina sada se pravilno primenjuju na prvi i drugi glas dijaloga tokom sinteze.
• Poboljšano je prepoznavanje kodiranja japanskih `.txt` datoteka: dodat je bezbedan Shift_JIS/CP932 rezervni način za slučajeve izobličenog teksta, uz očuvanje postojećeg ponašanja za UTF, dijakritiku i kineski.
• Interno bezbednosno refaktorisanje: funkcije su prebačene na bezbedne implementacije gde god je moguće i značajno je smanjen broj redova sa unsafe kodom.

Verzija 0.6.7 – 2026-03-02
Poboljšanja
• Program sada može masovno da izvrši „Zameni sve” na velikim datotekama sa veoma velikim brojem zamena.
• Ažuriran je poljski prevod, zahvaljujući DJ Graco.
• Dodata je litvanska lokalizacija.
• Dodata je kineska lokalizacija.
• Od sada će se česte beta verzije objavljivati u odeljku Releases projekta kako bi korisnici mogli da testiraju promene pre sledećeg stabilnog izdanja.
• Dodata je prečica `Ctrl+.` za unos znaka tri tačke (…).
• Poboljšana je podrška za poglavlja podkasta: kretanje kroz poglavlja sada pouzdanije radi, uključujući direktne/strimovane epizode u kojima poglavlja nisu ugrađena u MP3, korišćenjem metapodataka iz feed/URL rezervnih izvora kada su dostupni. Dodate su prečice `Ctrl+Alt+PageUp` za prethodno i `Ctrl+Alt+PageDown` za sledeće poglavlje.
• Reorganizovane su izlazne fascikle Sonarpada pod `Documents\\Sonarpad`: datoteke se sada čuvaju u posebnim podfasciklama `audiobooks`, `documents`, `recordings` i `media`, uz automatsku migraciju sa starih putanja.
• Poboljšana je podrška za veoma velike tekstualne datoteke, uključujući 60 MB: otvaranje i kretanje red po red su glađi, posebno sa čitačima ekrana.
• Ažurirani su vodiči za sve jezike i osveženi lokalizacioni resursi širom aplikacije, uključujući tekstove za donacije i NSIS prevode instalacije (novi pojednostavljeni kineski i litvanski tekstovi, kao i dovršen ukrajinski prevod instalera).
• Dodata je globalna mrežna proxy podrška (HTTP/HTTPS i SOCKS5/SOCKS5H) za onlajn funkcije, uz proveru proxy-ja pri čuvanju Opcija: neispravni proxy-ji se prijavljuju i automatski uklanjaju.
• Dodata je nova radnja u Alatima: „Reprodukuj striming audio...”, koja omogućava lepljenje URL-a (YouTube ili direktan medijski link), izbor izlaznog formata i profila kvaliteta/bit-brzine, uključujući originalni kvalitet/bit-brzinu za MP3 i MP4, i direktnu reprodukciju u Sonarpad audio-plejeru.
• Dodata je podrška za sistemski taster Play/Pause na slušalicama/tastaturama: sada kontroliše i medijsku reprodukciju i pauzu/nastavak čitanja teksta, uz prioritet medija ako su oba aktivna.
• U Datoteka > Nedavne datoteke dodata je nova stavka „Obriši nedavne datoteke” za brzo pražnjenje liste.
• Proširene su opcije audio bit-brzine u Konvertuj audio i podešavanjima snimanja podkasta: dodate su niže vrednosti 64/96 kbps i MP3 do 320 kbps, uz usklađenu proveru i enkodere.
• Opcije deljenja audio-knjige po vremenu proširene su do 60 minuta.
• Poboljšano je deljenje audio-knjiga na delove: korisnik sada može ručno uneti broj delova uz proveru od 1 do 100.
• Dodata je nova opcija Prikaz > Režim samo za čitanje koja zaključava tekst editora od slučajnih izmena, a dokument ostaje potpuno čitljiv i navigabilan.
• Dodata je pristupačna traka napretka tokom ažuriranja programa kako bi čitači ekrana mogli da prate preuzimanje u realnom vremenu.
• Dodata je nova tiha statusna traka glavnog prozora koja prikazuje znakove, reči i red/kolonu, na primer „Znakovi (sa razmacima): 11. | Reči: 2. | Red 1, Kol 12”, bez ometanja NVDA fokusa.
• U meni Prikaz dodat je prekidač Prelamanje redova kako bi se brzo uključilo ili isključilo bez otvaranja Opcija.
• U Redigovanje > Tekst dodate su radnje za uvlačenje/izvlačenje sa prečicama Ctrl+Shift+. i Ctrl+Shift+, jer kada je uključeno „Prikaži glasove u editoru” taster Tab služi za navigaciju panelom glasova.
• Dodati su lokalizovani datum i vreme u RSS člancima i epizodama podkasta, prilagođeni jeziku interfejsa.
• U RSS kontekstni meni dodata je radnja za deljenje izabranog članka e-poštom.
• U Opcije > RSS i podkast dodate su detaljne opcije potvrde brisanja: RSS (kanal/članak/oboje/ništa) i podkasti (podkast/epizoda/oboje/ništa).
• Dodato je podesivo brzo kopiranje RSS-a sa Ctrl+C (Opcije > RSS i podkast): kopiranje naslova, URL-a, sadržaja članka ili svega zajedno.
• Objedinjeno je dodavanje RSS izvora: „Dodaj izvor” sada prihvata i direktne URL adrese feed-a i ključne reči, za koje automatski pravi Google News RSS, pa posebna radnja pretrage po ključnoj reči više nije potrebna.
• Ctrl+A sada najavljuje završetak radi jasnije povratne informacije čitača ekrana.
• Dodata je prečica Shift+F3 za „Pronađi prethodno” u meniju Redigovanje, kao dopuna F3 „Pronađi sledeće”.
• Poboljšane su poruke o zameni sa pravilnim oblicima jednine/množine, npr. „1 zamena izvršena” naspram „2 zamene izvršene”.
• U prozor rečnika dodat je izbor jezika pretrage, sa podrazumevanim Automatski (jezik interfejsa) i mogućnošću ručnog izbora.
• U Opcije je dodat novi tab Prečice za prilagođavanje kombinacija tastera, uz otkrivanje konflikta i upozorenje kada je prečica već dodeljena drugoj radnji.
• Dodata je početna podrška za argumente komandne linije: `-h`/`--help` prikazuju upotrebu, a `--version` verziju programa.
• Poboljšana je jasnoća ručnog podešavanja brzine i visine: polja koriste skalu centriranu na 100, gde 100 odgovara normalnoj vrednosti.
• Poboljšan je izbor Microsoft glasova u Opcije > Glas i panelu Glas u editoru: dodat je lokalizovan izbor jezika za filtriranje glasova, dok režim samo višejezičnih glasova ostaje jedna negrupisana lista, a izbor jezika je sakriven kada je taj režim uključen.
• U Opcije > Glas dodata je konfiguracija glasova za dijalog sa potpunom Tab navigacijom, istim modelom glasova kao glavni interfejs (motor, Edge filter jezika, glas i označena brzina/visina/jačina); dodat je i opcioni drugi glas dijaloga sa istim kontrolama za naizmenične dijaloge. Pravila se čuvaju u `.ini` konfiguraciji pa se tekst dokumenta ne menja.
• Poboljšan je naziv Undo radnje: Redigovanje > Poništi sada prikazuje šta će biti poništeno, na primer izmena teksta, citiranje/uklanjanje citata ili umetanje oznake glasa, a ostaje onemogućeno kada nema šta da se poništi.
Ispravke grešaka
• Ispravljeno je otvaranje RTF datoteka: `.rtf` dokumenti se sada analiziraju i prikazuju kao običan čitljiv tekst umesto sirovog RTF koda, npr. `{\\rtf1...}`.
• Ispravljeno je otvaranje kineskih tekstualnih datoteka kodiranih u GB18030/GBK: Sonarpad ih sada pravilno prepoznaje i dekoduje, bez nečitljivog izlaza.
• Poboljšano je pravljenje M4B audio-knjiga sa metapodacima i markerima poglavlja; ispravljen je problem ubrzanog, visokog „veveričjeg” zvuka u generisanim M4B datotekama.
• Ispravljen je prikaz bit-brzine u dijalogu za čuvanje audio-knjige: uklonjene su fiksne italijanske oznake i dodato 64 kbps u opcije.
• Ispravljeno je Sačuvaj sve (Ctrl+Shift+S): svi otvoreni izmenjeni dokumenti sada se pouzdano pronalaze, uključujući nesačuvane/nove tabove, i svaki se pravilno čuva ili otvara Sačuvaj kao kada je potrebno.
• Ispravljen je redosled Google News RSS stavki: članci se sada prikazuju po opadajućem datumu objave, najnoviji prvo, kada su datumi dostupni.
• Ispravljeno je povezivanje NVDA oznaka u prozoru rečnika: polje pretrage i izbor jezika sada najavljuju ispravne oznake.
• Ispravljeno je rukovanje tastaturom u prozoru Svojstva RSS/Podkasta: Tab/Shift+Tab dolazi do dugmeta OK, Enter aktivira OK, Esc bezbedno zatvara prozor i fokus se pravilno vraća na RSS/Podkast listu.
• Ispravljena je istorija poništavanja RSS/Podkast radnji: Ctrl+Z sada podržava višestruko poništavanje uklanjanja članaka/epizoda i izvora, ne samo poslednje radnje.
• Poboljšana je povratna informacija pri uklanjanju RSS/Podkast stavki jasnim statusnim porukama (RSS uklonjen, RSS članak uklonjen, epizoda podkasta uklonjena).
• Poboljšano je ponašanje fokusa posle brisanja/poništavanja u RSS/Podkast: RSS sada pouzdano fokusira prvi kanal kada je potrebno i izbegava ponovljene najave čitača ekrana pri odloženom ponovnom izboru.

Verzija 0.6.6 – 2026-02-13
Poboljšanja
• U meni Redigovanje dodata je funkcija „Automatski formatiraj za TTS” za brzo pripremanje teksta za govor, uklanjanjem Markdown-a/citata i spajanjem prelomljenih redova.
• Poboljšano je umetanje oznaka glasa: kada je tekst označen, oznake se sada pravilno primenjuju i na jednoredne i na višeredne izbore.
• U Audio podešavanjima dodata je opcija za podrazumevanu fasciklu za čuvanje audio-knjiga (podrazumevano: Documents\\Sonarpad Audiobooks).
• U dijalogu za čuvanje audio-knjige, kada je uključeno deljenje, dodata je nova podrazumevano uključena opcija da se napravi posebna podfascikla za delove, radi urednije organizacije izlaza.
• Izvoz audio-knjiga sada čuva MP3 u stereo formatu sa bit-brzinom koju korisnik izabere za Edge, SAPI5 i SAPI4 glasove.
• Dodata je podrška za 32-bitne SAPI5 glasove preko bridge-a, pa se u Sonarpadu mogu koristiti i glasovi dostupni samo u 32-bitnim motorima.
• Funkcije glasa reorganizovane su u poseban meni „Glas i audio”, a „Konvertuj audio” je dodat/pojašnjen kao alat za konverziju svih podržanih medijskih datoteka u MP3, AAC, OGG, Opus, FLAC, WAV i AIFF.
• Dodato je uklanjanje pojedinačnih RSS članaka i epizoda podkasta (Delete + kontekstni meni sa potvrdom), bez uklanjanja čitavog RSS/podkast izvora, kao i poništavanje poslednjeg uklanjanja, bilo pojedinačne stavke ili celog izvora.
• U RSS prozoru dodat je izvoz RSS kanala u OPML, tako da se trenutni RSS izvori mogu lako sačuvati i ponovo uvesti.
• U RSS prozoru dodata je „Pretraga RSS-a po ključnoj reči”: unos ključne reči automatski generiše Google News RSS URL i otvara unapred popunjen dijalog za dodavanje izvora, pa se kanal po ključnoj reči pravi u jednom koraku.
• Dodata je srpska lokalizacija, zahvaljujući Mila Kuran.
• Dodata je ukrajinska lokalizacija, zahvaljujući Ivan Shtefuriak.
• Dodato je otvaranje više medijskih datoteka: izbor/otvaranje više datoteka sada pravi red za reprodukciju umesto da zameni trenutnu datoteku.
• Dodate su promenljive prečice za premotavanje tokom reprodukcije: uz osnovni skok od 1 minuta, Levo/Desno pomera 60 s, Shift+Levo/Desno 20 s, a Ctrl+Levo/Desno 3 minuta.
• Dodate su prečice za prethodnu/sledeću numeru u plejeru: Ctrl+PageUp i Ctrl+PageDown.
• Dodato je „Resetuj jačinu” i radnje resetovanja grupisane su u poseban podmeni „Resetuj” u meniju Reprodukcija, uz „Resetuj brzinu” i „Resetuj visinu”.
• Poboljšan je instalacioni program: setup.exe sada omogućava izbor između povezivanja svih podržanih tipova datoteka i ručnog izbora ekstenzija; MSI sada prikazuje opcije povezivanja po ekstenziji u stablu funkcija, dok su sve podrazumevano uključene.
• Dodat je novi meni „Prozor” sa stavkom „Otvoreni dokumenti...” za brzo prebacivanje na bilo koju trenutno otvorenu datoteku.
• Ažurirano Prikaz > Font: stari birač zamenjen je brzim podmenijem uobičajenih fontova (Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman, Georgia), uz zadržavanje trenutne veličine teksta.
• Poboljšane su RSS/Podkast najave dvostrukim modelom statusa: čvorovi izvora najavljuju „nove stavke” kada ima ažuriranja, dok pojedinačni RSS članci i epizode najavljuju „nepročitano”/„nepreslušano”. Ovo se može isključiti u Opcijama.
Ispravke grešaka
• Ispravljeno je izdvajanje teksta iz EPUB knjiga sa ugrađenim HTML komentarima (<!-- ... -->): tekst poglavlja se sada pravilno analizira umesto da bude delimično ili potpuno preskočen.
• Ispravljene su španske Wiktionary pretrage i keš rečnika: španske odrednice poput „agua” sada se učitavaju pravilno, a stari keširani rezultati „Word not found” više se ne koriste.
• Ispravljeno je kodiranje pri uvozu RSS članaka iz nekih španskih izvora, npr. El Mundo: akcentovana slova i „ñ” sada se pravilno čuvaju u privremenom editoru.
• Ispravljeno je ANSI dekodiranje srednjoevropskih datoteka, npr. čeških/poljskih: Sonarpad sada bolje razlikuje UTF-8 i ANSI i bira odgovarajuću kodnu stranicu, uključujući Windows-1250, kako bi se izbegla oštećena dijakritika.
• Ispravljeno je čuvanje RSS izvora sa parametrima u URL-u, npr. `rss.aspx?c=...`: oni se sada pravilno čuvaju i vraćaju posle ponovnog pokretanja Sonarpada.
• Ispravljeno je otvaranje Google Drive pokazivačkih datoteka (`.gdoc`, `.gsheet`, `.gslides`) iz kontekstnog menija Explorera: kada direktno čitanje ne uspe sa „Incorrect function (os error 1)”, Sonarpad prelazi na sistemsko otvaranje pa se dokument ipak pravilno otvara.
• Ispravljeno je čitanje starih Excel 2010 `.xls` datoteka: stare binarne Excel datoteke sada se pravilno prepoznaju i dekodiraju umesto da prikazuju nečitljiv tekst, npr. `ÐÏ_à¡±...`.
• Ispravljen je tok najava provere pravopisa: pogrešno napisane reči ponovo se najavljuju pri kasnijem pregledu teksta, a ista greška se ponovo prijavljuje ako se obriše i ponovo otkuca.
• Ispravljene su radnje nad redovima (npr. Ctrl+Q / Ctrl+Shift+Q, sortiranje/obrtanje/jedinstveni/spajanje redova): izbor jednog reda sa Shift+Strelica dole više ne spaja niti skraćuje susedne redove.
• Ispravljeno je višeredno ponašanje radnji nad redovima: RichEdit izbori sa samo CR separatorima sada se pravilno normalizuju, tako da svi označeni redovi budu obrađeni bez odsecanja prvih znakova.
• Proširena je normalizacija TTS ulaza za vidljive simbole razmaka (␠/U+2420, ␣/U+2423, ␉/U+2409, ␊/U+240A, ␍/U+240D, ␤/U+2424) kako bi se sprečilo ponavljanje pasusa sa višejezičnim glasovima.
• Doterano je čišćenje Edge TTS teksta jedinstvenim tokom provere: neobični/nevidljivi razmaci se normalizuju, dugi nizovi interpunkcije poput „...”, „!!!”, „???” skraćuju se, a delovi sastavljeni samo od interpunkcije preskaču se da bi se sprečile petlje reprodukcije.
• Ispravljena je najava vremena reprodukcije (Ctrl+I) za MP3/podkast strimove: trenutno vreme sada je ograničeno trajanjem numere, a reprodukcija se automatski zaustavlja ako pozicija pređe kraj.
• Proširena je lokalizacija instalera: setup.exe sada uključuje dodatne jezike (češki, poljski, francuski, srpski), dok MSI ostaje jedan en-US paket kako bi se izbegla zabuna pri izdavanju.
• Ispravljeno je čišćenje stavki kontekstnog menija pri deinstalaciji: „Otvori pomoću Sonarpada” sada se pouzdano uklanja, uključujući stare scenarije registra.
• Ispravljena je pouzdanost SAPI5 pauze/nastavka: F4 sada pravilno pauzira, a nastavak se vraća na očekivanu poziciju umesto da krene od početka.
• Ispravljen je tok pauza + premotavanje + nastavak za medije: nakon pauze i pomeranja Levo/Desno, Space sada pouzdano nastavlja sa trenutne pozicije umesto da zaustavi ili ponovo pokrene od početka.

Verzija 0.6.5 – 2026-02-07
Poboljšanja
• Poboljšan je španski prevod, zahvaljujući Arturo Fernandez Rivas.
• Dodata je opcija za deljenje EPUB audio-knjiga po poglavljima.
• RSS uvozi sada koriste poseban privremeni tab sa lokalizovanim naslovom; „Sačuvaj kao” pretvara ga u običan dokument.
• Poruke čitača ekrana sada se šalju i JAWS-u kada je dostupan.
Ispravke grešaka
• Čitanje od kursora (F5) sada počinje tačno na poziciji kursora. Ranije je moglo početi nekoliko redova iznad jer pomeraj kursora nije odgovarao CRLF/UTF-16 pozicijama.
• Ispravljen je problem sa iscrtavanjem gde je kucanje preko izbora moglo privremeno da sakrije prethodni tekst dok se izbor ne pomeri.
• Ispravljena je analiza EPUB poglavlja tako da naslovnice ili stranice koje sadrže samo sliku više ne proizvode izgovoreni CSS, npr. „padding”, niti naslove „Sconosciuto”.
• Ispravljeno je vremensko deljenje EPUB audio-knjiga sa Edge TTS kada prazni/preveliki delovi izazovu „Edge audio not sent”.
• RSS članci sada dekodiraju HTML entitete, npr. &quot;, &amp;, &lt;, &gt;.
• Sačuvaj/Sačuvaj kao sada predlaže postojeće ime datoteke pri čuvanju formata koji se ne može prepisati, npr. EPUB, umesto prvog reda teksta.
• Ispravljena je greška gde podkasti sa novim epizodama nisu najavljivani kao nepreslušani, a „Unheard” je preimenovan u „Unplayed” radi profesionalnijeg naziva.

Verzija 0.6.4 – 2026-02-05
Poboljšanja
• Program je preimenovan u Sonarpad kako bi zvuk i audio bili naglašeni kao glavni fokus.
• U meni Reprodukcija dodat je izbor audio trake za medijske datoteke sa više audio traka, npr. MKV sa više jezika.
• Podkasti sada jasno označavaju nepreslušane epizode prefiksom „Unheard” ispred imena.
• Novo prebacivanje glasova u tekstu pomoću oznaka. Primeri:
  - Microsoft glasovi (Edge): <voice edge it-IT-IsabellaNeural>Hello</voice>
  - SAPI5 glasovi: <voice sapi5 Microsoft Helena Desktop>Hello</voice>
  - SAPI4 glasovi: <voice sapi4 #1>Hello</voice>
  - Sa brzinom/visinom/jačinom: <voice edge it-IT-ElsaNeural speed=-20 pitch=-5 volume=-10>Hello</voice>
• Proširene su kategorije podkasta.
• Poboljšano je čitanje PDF-a sa automatskim rezervnim PDFium načinom.
• Poboljšan je analizator članaka za slučajeve u kojima sadržaj nije bio pročitan u celosti.
• U meni Reprodukcija dodat je reset visine tona.
• U kontekstni meni dodata je opcija za pravljenje audio-knjige od označenog teksta.
• Dodato je deljenje audio-knjige po trajanju, uz mogućnost izbora imena prve datoteke.
• Lokalizovana je oznaka autora pri čitanju članaka, npr. „by”, „di”, „par”.
• Dodate su opcije uvlačenja (tabovi/razmaci sa širinom) i Tab/Shift+Tab za uvlačenje/izvlačenje označenih redova.
• Ispravljeno je Markdown čišćenje za `*` oznake liste kada je čuvanje oznaka isključeno.
• Dodata je opcija za korišćenje starog imena „Novapad” u naslovu prozora i prečicama Start menija.
Ispravke grešaka
• Ispravljena je greška zbog koje su SAPI4 audio-knjige mogle biti napravljene drugačije od očekivanog.
• Ispravljena je greška gde premotavanje iza kraja medijske datoteke ponovo pokreće reprodukciju od početka.
• Prozor Pretraga u datotekama: Enter na rezultatu sada otvara tačno na poziciji isečka, a Esc vraća na rezultate.
• Prozor Opcije: poboljšan je vizuelni raspored kartica Opšte, Glas, Editor i Audio da bi se sprečili nedostajući ili odsečeni elementi.
• Ispravljen je problem sa obeleživačem pri promeni brzine reprodukcije.
• Ispravljeno je nepravilno prikazivanje Podcast Index kategorija.
• Ispravljeno je da apostrofi prekidaju čitanje uklanjanjem odvojenog čitanja dijaloga; umesto toga koriste se oznake glasa.

Verzija 0.6.3 – 2026-01-30
Poboljšanja
• Poboljšano je prepoznavanje mikrofona.
• Dodata je trenutna reprodukcija za sve formate.
Ispravke grešaka
• Ispravljen je pad u prozoru kategorija podkasta.

Verzija 0.6.2 – 2026-01-30
Nove funkcije
• Dodata je podrška za izvršavanje datoteka (Shift+F5). Korisnik može u Opcijama izabrati interpreter, npr. Python, pronaći ga na računaru i sa Shift+F5 pokrenuti trenutnu skriptu. HTML datoteke otvaraju se u pregledaču.
• Dodata je podrška za Google Docs pokazivačke datoteke (.gdoc, .gsheet, .gslides), koje se automatski otvaraju u podrazumevanom pregledaču.
• Dodata je podrška za M4B format audio-knjiga (Apple/AAC).
• U kontekstni meni rezultata pretrage podkasta dodato je „Prikaži epizode” za pregled i reprodukciju bez pretplate.
• Dodata je funkcija „Idi na red” (meni Redigovanje ili Ctrl+J) za brzo skakanje na određeni broj reda.
• Dodate su opcije kontekstnog menija za sortiranje RSS kanala i podkasta abecedno ili po datumu.
• Dodati su podrazumevani vijetnamski RSS kanali.
• U dijalog za snimanje dodato je polje za test mikrofona radi provere nivoa pre početka.
• U kontekstni meni epizoda podkasta dodato je „Prikaži opis”.
• Dodata je podrška za proširene audio/video formate preko FFmpeg-a: mkv, avi, mov, m4v, webm, mpg, ts, wmv, flv, vob, 3gp, flac, ogg, wma, aiff.
• Dodata je sinhronizovana reprodukcija titlova (srt, vtt, ass, sub, sbv, lrc, smi) preko NVDA ili izabranog glasa. Program traži titl sa istim imenom kao medijska datoteka. U meni Reprodukcija dodate su opcije „Uvezi titlove” i „Ukloni titlove” za datoteke sa različitim imenima.
• Dodate su asocijacije za sve nove podržane audio/video formate u kontekstni meni „Otvori pomoću Sonarpada”.
• Dodato je podešavanje visine tona za svaku datoteku.
• U Opštim podešavanjima dodata je opcija za uključivanje/isključivanje anonimnih izveštaja o greškama. U meni Pomoć dodata je stavka za pravljenje dijagnostičkog ZIP-a.
• Dodata je opcija za korišćenje drugog glasa za dijaloge, pri čitanju uživo i pri pravljenju audio-knjige.
• Dodat je pregled kategorija podkasta za istraživanje po kategorijama kao što su posao, umetnost, sport itd.
Poboljšanja
• Otvaranje audio/video datoteke iz Explorera sada direktno otvara prikaz plejera umesto tekstualnog editora.
• Uklonjen je OCR upit za nepristupačne PDF-ove; OCR se sada izvršava automatski radi bržeg i jednostavnijeg rada.
• Poboljšan je Pristupačni terminal: NVDA čitanje sada pamti poslednji pročitani red radi boljeg kontinuiteta.
• SAPI4: pravljenje audio-knjige sada je potpuno paralelizovano i gotovo trenutno. Dodat je upit za broj istovremenih procesa.
• SAPI4: uklonjeno je usko grlo WAV-u-MP3 konverzije paralelnom konverzijom delova tokom sinteze.
• SAPI4: poboljšano je rukovanje greškama i automatsko čišćenje privremenih datoteka.
• Dijalog za pretragu: „Regex” je preimenovan u „Regularni izraz” radi jasnoće i dodati su nedostajući prevodi opcija pretrage.
• M4B audio-knjige: poboljšano je rukovanje izlazom; deljenje po delovima/markerima sada pravi jednu M4B datoteku sa ispravnim metapodacima poglavlja, uključujući naslov i autora.
• Plejer: ispravljena je preciznost obeleživača i najave vremena kada brzina nije 1,0x.
• Vraćena je navigacija Ctrl+Tab i Ctrl+Shift+Tab u Opcijama.
• U meni Reprodukcija dodata je opcija za trenutno vraćanje brzine na Normalno (1,0x).
• Ažurirane su sve zavisnosti na najnovije verzije radi boljih performansi i stabilnosti.
• FFmpeg je integrisan sa dinamičkim DLL učitavanjem radi kompatibilnosti bez blokiranja pokretanja.
• Ažurirani su filteri za preuzimanje podkasta da uključe nove audio/video formate.
• Sprečeno je da Ctrl+S čuva audio/video datoteke kako bi se izbeglo oštećenje.
• Poboljšan je uvoz YouTube transkripata kako bi bio robusniji i otporniji.
• Poboljšana je pouzdanost deljenja audio-knjiga na delove, bez gubitka teksta.
• Instalacioni program je sada potpuno višejezičan i podržava italijanski, engleski, španski, portugalski, švedski i vijetnamski prema jeziku sistema. Engleski je podrazumevan za nepodržane sisteme.
• Kategorije podkasta: Enter na kategoriji sada potvrđuje izbor, isto kao dugme OK.
• Poboljšan je sistem otkrivanja zaglavljivanja kako bi se izbegli lažni alarmi dok su otvoreni modalni dijalozi, poput grešaka ili „tekst nije pronađen”.
Ispravke
• Ispravljena je greška zbog koje se dnevnik izmena nije otvarao pri pokretanju.
• Ispravljena je greška gde OCR upit nije bio prikazan za nepristupačne PDF-ove otvorene iz Explorera.
• Ispravljena je greška pri pokretanju koja je mogla izazvati gubitak fokusa ili zatvaranje prozora odmah nakon otvaranja.
• Ispravljena je kritična greška u regex pretrazi koja je sprečavala pronalaženje teksta, uključujući probleme sa „Pretraži od početka” i opcijom „Tačka odgovara novom redu” sa Windows završecima redova.
Lokalizacija
• Dodat je poljski prevod.
• Dodat je francuski prevod.
• Dodat je češki prevod, zahvaljujući Radek Žalud i Jiri Holzinger.

Verzija 0.6.1 – 2026-01-20
Ispravke
• Ispravljena je greška gde uključivanje „Prikaži glasove u editoru” zaustavlja reprodukciju podkasta.
• Ispravljen je problem gde neki podkasti nisu mogli biti dodati preko URL-a jer je URL bio skraćen.
• Ispravljena je greška gde obični URL-ovi više nisu mogli da se dodaju u RSS funkciju.
• Ispravljeno je da se opcija jezika Wikipedije prikazuje više puta na različitim karticama podešavanja.
• Uklonjeno je pravljenje debug datoteka koje su se greškom stvarale i u release režimu.
Poboljšanja
• Poboljšana je podrška za Microsoft glasove, koji sada koriste poseban način reprodukcije sa drugačijim user agent-om.
• Dodata je podrška za MP4 datoteke.

Verzija 0.6.0 – 2026-01-20
Nove funkcije
• Dodat je proveravač pravopisa. Iz kontekstnog menija može se proveriti da li je trenutna reč ispravna i dobiti predlozi ako nije.
• Dodati su uvoz i izvoz podkasta preko OPML datoteka.
• Pored iTunes-a dodata je Podcast Index pretraga. Korisnici mogu uneti besplatan API ključ i tajnu koji se generišu samo pomoću e-adrese.
• Dodata je podrška za SAPI4 glasove, za čitanje uživo i pravljenje audio-knjiga.
• Dodat je automatski OCR rezervni način za nepristupačne PDF-ove: kada nema teksta za izdvajanje, dokument se prepoznaje OCR-om.
• Dodat je rečnik preko Wiktionary-ja. Pritiskom na taster Applications prikazuju se definicije i, kada postoje, sinonimi i prevodi na druge jezike.
• Dodat je uvoz članaka sa Wikipedije sa pretragom, izborom rezultata i direktnim uvozom u editor.
• U RSS modul dodata je prečica Shift+Enter za direktno otvaranje članka na originalnom sajtu.
Poboljšanja
• Izbor mikrofona sada se uvek poštuje.
• U prozoru podkasta, Enter na epizodi odmah preko NVDA najavljuje „učitavanje” kao potvrdu radnje.
• U rezultatima pretrage podkasta Enter sada pretplaćuje na izabrani podkast.
• Ispravljene su i poboljšane oznake za Ctrl+Shift+O i Podkast Ctrl+Shift+P prečice.
• Brzina i jačina reprodukcije sada se čuvaju u podešavanjima i važe kroz sve audio datoteke.
• Dodata je posebna keš fascikla za epizode podkasta. Epizode se mogu zadržati preko „Zadrži podkast” u meniju Reprodukcija. Keš se automatski čisti kada pređe veličinu koju korisnik zada (Opcije → Audio).
• Značajno je poboljšano preuzimanje RSS članaka korišćenjem libcurl imitacije Chrome i iPhone profila, uz kompatibilnost sa približno 99% sajtova.
• Dodat je status pročitano/nepročitano za RSS članke sa jasnom oznakom u RSS listi.
• Zameni sve sada prijavljuje broj izvršenih zamena.
• Dodato je dugme Obriši podkast pri navigaciji bibliotekom podkasta pomoću Tab-a.
Ispravke
• Iz menija Pomoć uklonjena je suvišna stavka „ažuriranje na čekanju”, pošto se ažuriranja već automatski obrađuju.
• Ispravljena je greška gde Ctrl+S na otvorenom MP3 fajlu čuva i oštećuje datoteku.
• Ispravljen je UI problem gde se „Paketne audio-knjige” prikazivalo kao „(B)… Ctrl+Shift+B”; suvišna oznaka je uklonjena.
• Ispravljeni su pametni navodnici: kada su uključeni, obični navodnici sada se pravilno zamenjuju pametnim.
• Ispravljena je greška gde „Idi na obeleživač” vraća brzinu reprodukcije na 1,0.
• Ispravljen je problem gde se već preuzete epizode podkasta ponovo preuzimaju umesto da se koristi keširana verzija.
Prečice na tastaturi
• F1 sada otvara vodič Pomoći.
• F2 sada proverava ažuriranja.
• F7 / F8 sada prelaze na prethodnu ili sledeću pravopisnu grešku.
• F9 / F10 sada brzo menjaju omiljene glasove.
Poboljšanja za programere
• Greške se više ne odbacuju tiho: svi `let _ =` obrasci su uklonjeni i greške se sada eksplicitno prosleđuju, beleže ili obrađuju rezervnim načinima.
• Projekat sada ne prolazi kompilaciju ako postoje upozorenja: i cargo check i cargo clippy moraju proći bez upozorenja, uz stroža lint pravila i uklanjanje `allow` gde god je moguće.
• Uklonjene su sopstvene implementacije poput strlen / wcslen pomoćnih funkcija. Dužine stringova i UTF-16 bafera sada se dobijaju iz podataka kojima upravlja Rust umesto skeniranjem memorije.
• DLL rukovanje je očišćeno i objedinjeno oko libloading, bez sopstvene logike učitavanja i PE analize.
• Uklonjeni su ručno pisani parseri bajtova; sva parsiranja koriste standardne from_le_bytes / from_be_bytes na proverenim isečcima.
Ove promene smanjuju nepotrebnu upotrebu unsafe koda, uklanjaju moguće nedefinisano ponašanje i čine kod idiomatičnijim, robusnijim i lakšim za održavanje.

Verzija 0.5.9 - 2026-01-13
Nove funkcije
• Dodato je preraspoređivanje RSS izvora iz kontekstnog menija (gore/dole/na poziciju) uz proveru nevažeće pozicije.
• Dodat je kontekstni meni članka sa otvaranjem originalnog sajta i deljenjem preko WhatsApp-a, Facebook-a i X-a.
• Dodata je Esc prečica za povratak iz uvezenih članaka na RSS listu.
• Dodat je režim podkasta: pretraga, pretplata, slušanje; preraspoređivanje pretplata; Esc zaustavlja reprodukciju i vraća na listu; Enter na epizodi pokreće reprodukciju.
• Dodata je kontrola brzine reprodukcije za podkaste i MP3 datoteke.
• Dodat je Ctrl+T za skok na određeno vreme.
• Dodato je dugme za pregled glasa posle izbora jačine.
• Dodata je regex pretraga i zamena u Notepad++ stilu.
• Dodat je RSS uvoz iz OPML i TXT datoteka.
• Dodata je opcija za uključivanje „Otvori pomoću Sonarpada” u File Exploreru, uključujući prenosive verzije.
Poboljšanja
• Poboljšan je izbor brzine/visine/jačine glasa uz poštovanje maksimalnih TTS ograničenja.
• Razna RSS poboljšanja omogućavaju preuzimanje svih članaka bez pomeranja NVDA fokusa tokom ažuriranja.
• Poboljšana je audio reprodukcija posebnim menijem, Ctrl+I najavom vremena i jačinom do 300%.
• Dodate su prečice koje su nedostajale za neke funkcije.
• Meni Redigovanje reorganizovan je podmenijem za čišćenje teksta.
• Opcije su reorganizovane u kartice sa Ctrl+Tab i Ctrl+Shift+Tab navigacijom.
• RSS čitač sada preuzima ceo sadržaj članka, kao u pregledaču.
Ispravke
• Ispravljeno je Markdown čišćenje koje je uklanjalo brojeve na početku redova.
• Ispravljeno je da AltGr+Z pokreće poništavanje.
• Ispravljeno je otkazivanje snimanja audio-knjige tako da se brzo zaustavlja.
Lokalizacija
• Dodat je vijetnamski prevod, zahvaljujući Anh Đức Nguyễn.

Verzija 0.5.8 - 2026-01-10
Nove funkcije
• Dodata je kontrola jačine mikrofona i sistemskog zvuka pri snimanju podkasta.
• Dodata je funkcija za uvoz članaka sa sajtova ili RSS kanala, uključujući najvažnije kanale za svaki jezik.
• Dodata je funkcija za uklanjanje svih obeleživača iz trenutne datoteke.
• Dodata je funkcija za uklanjanje duplih redova i uzastopnih duplih redova.
• Dodata je funkcija za zatvaranje svih tabova ili prozora osim trenutnog.
• U meni Pomoć dodata je stavka Donacije za sve jezike.
Poboljšanja
• Poboljšan je pristupačni terminal kako bi se sprečili neki padovi.
• Poboljšani su i ispravljeni pristupni tasteri i prečice širom aplikacije.
• Ispravljen je problem gde zatvaranje prozora audio reprodukcije nije zaustavljalo reprodukciju.
• Dodati su dijalozi za potvrdu važnih radnji, npr. uklanjanje duplih redova, uklanjanje crtica na kraju reda i uklanjanje svih obeleživača. Dijalog se ne prikazuje kada radnja nije primenljiva.
• Dodata je mogućnost brisanja RSS kanala/sajtova iz biblioteke izborom i pritiskom na Delete.
• U RSS prozoru dodat je kontekstni meni za uređivanje ili brisanje RSS kanala/sajtova.
• Uklonjeno je podešavanje za premeštanje konfiguracije u trenutnu fasciklu; aplikacija to sada automatski rešava prema lokaciji: ako se exe fascikla zove „sonarpad portable” ili je exe na prenosivom disku, podešavanja idu u `config` pored exe-a; inače u `%APPDATA%\\Sonarpad`, uz rezervni exe `config` ako željena lokacija nije upisiva.

Verzija 0.5.7 - 2026-01-05
Nove funkcije
• Dodata je funkcija Paketne audio-knjige za konverziju više datoteka/fascikli odjednom.
• Dodata je podrška za Markdown datoteke (.md).
• Dodat je izbor kodiranja pri otvaranju tekstualnih datoteka.
• U pristupačnom terminalu dodata je opcija da NVDA najavljuje nove redove.
Poboljšanja
• Snimanje audio-knjiga sada čuva direktno u MP3 kada je taj format izabran.
• Korisnik sada može izabrati poziciju zvezdice (*) za nesačuvane izmene u naslovu prozora.
• Poboljšana je robusnost sistema ažuriranja u različitim scenarijima.
• U meni Redigovanje dodato je „Ukloni crtice” za ispravljanje OCR preloma na kraju reda.

Verzija 0.5.6 - 2026-01-04
Ispravke
  Poboljšana je Pretraga u datotekama tako da Enter otvara datoteku tačno na izabranom isečku.
Poboljšanja
  Dodata je podrška za PPT/PPTX (otvaranje kao tekst).
  Otvaranje netekstualnih formata sada čuva kao .txt da bi se izbeglo oštećenje formatiranja (PDF/DOC/DOCX/EPUB/HTML/PPT/PPTX).
  Dodato je snimanje podkasta iz mikrofona i sistemskog zvuka (meni Datoteka, Ctrl+Shift+R).

Verzija 0.5.5 – 2026-01-03
Nove funkcije
• Dodat je pristupačni terminal optimizovan za veliki izlaz i čitače ekrana (Ctrl+Shift+P).
• Dodato je podešavanje za čuvanje korisničkih podešavanja u trenutnoj fascikli (prenosivi režim).
Ispravke
• Poboljšani su isečci Pretrage u datotekama kako bi pregled ostao poravnat sa pogotkom.

Verzija 0.5.4 – 2026-01-03
Poboljšanja
• Ispravljeno je Normalizuj razmake (Ctrl+Shift+Enter).
• Dodata je podrška za HTML/HTM (otvaranje kao tekst).

Verzija 0.5.3 – 2026-01-02
Nove funkcije
• Dodata je Pretraga u datotekama.
• Dodati su novi alati za tekst: Normalizuj razmake, Tvrdi prelom reda i Ukloni Markdown.
• Dodata je Statistika teksta (Alt+Y).
• U meni Redigovanje dodate su nove komande za liste:
• Poređaj stavke (Alt+Shift+O)
• Zadrži jedinstvene stavke (Alt+Shift+K)
• Obrni stavke (Alt+Shift+Z)
• Dodato je Citiraj / Ukloni citat iz redova (Ctrl+Q / Ctrl+Shift+Q).
Lokalizacija
• Dodata je španska lokalizacija.
• Dodata je portugalska lokalizacija.
Poboljšanja
• Kada je EPUB otvoren, Sačuvaj sada automatski prelazi na Sačuvaj kao i izvozi sadržaj u .txt kako bi se sprečilo oštećenje EPUB-a.

## 0.5.2 - 2026-01-01
- Dodat je dnevnik izmena.
- Dodate su opcije „Otvori pomoću Sonarpada” i povezivanje podržanih datoteka tokom instalacije.
- Poboljšana je lokalizacija poruka (greške, dijalozi, izvoz audio-knjiga).
- Dodat je izbor delova pri korišćenju „Podeli audio-knjigu prema tekstu”, sa opcijom „Zahtevaj marker na početku reda”.
- Dodat je uvoz YouTube transkripta sa izborom jezika, opcijom vremenskih oznaka i poboljšanim fokusom.

## 0.5.1 - 2025-12-31
- Automatska ažuriranja sa potvrdom, poboljšano rukovanje greškama i obaveštenja.
- Poboljšanja izvoza audio-knjiga (deljenje po tekstu, SAPI5/Media Foundation, napredne kontrole).
- TTS poboljšanja (pauza/nastavak, rečnik zamena, favoriti).
- Meni Prikaz i paneli glasova/favorita, boja i veličina teksta.
- Podrazumevani jezik prema sistemskom lokalitetu i poboljšanja lokalizacije.
- CI i Windows pakovanje (artefakti, MSI/NSIS, keš).

## 0.5.0 - 2025-12-27
- Modularno refaktorisanje (editor, rukovanje datotekama, meni, pretraga).
- Windows build/pakovanje workflow i ažuriranja README/licence.
- Ispravljena TAB navigacija u prozoru Pomoći.

## 0.5 - 2025-12-27
- Preliminarno povećanje verzije.

## 0.1.0 - 2025-12-25
- Prvo izdanje: struktura projekta i README.

10. Fixed segment reanalysis timing when mapping an independently analyzed mini-film back to the original movie. Sonarpad now applies the same small chunk-duration reconciliation used by the normal full analysis to descriptions, visual-evidence timestamps, and protected dialogue intervals, preventing progressive sub-second drift near the end of a chunk from moving narration onto speech.

