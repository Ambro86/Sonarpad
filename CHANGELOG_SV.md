# Ändringslogg

Version 0.9.8 – 2026-09-12

AI-ljudbeskrivning
1. Ett konservativt återställningsläge för flerkanalsexport har lagts till utan att ändra pipeline för filer som redan fungerar. Sonarpad använder fortfarande den ursprungliga kanallayouten först och fyller bara ut en ofullständig sista PCM-ram vid behov. Om en flerkanalig mellan-WAV ändå misslyckas eftersom antalet sampel inte är en multipel av antalet kanaler, försöker Sonarpad automatiskt om endast den exporten i stereo. Fungerande mono-, stereo- och flerkanalsexporter är oförändrade.

Mörkt tema och tillgänglighet
1. Åtgärdat ett problem där mörkt tema störde inbyggda Windows-dialoger och bekräftelsemeddelanden. Sonarpad använder inte längre sin anpassade dark-theme-subclass på systemdialogträd av typen `#32770`, så Öppna/Spara, diagnostikexport och MessageBox hanteras av Windows och får korrekt tangentbords-/NVDA-fokus. Livslängden för standardändelsen ZIP i diagnostikens Spara-dialog har också korrigerats.

Version 0.9.7 – 2026-09-11

AI-syntolkning
1. Lade till ett valfritt alternativ för att skapa ljudbeskrivningen i den ursprungliga videofilen. När det är aktiverat föredrar Sonarpad nu MP4: den ursprungliga videoströmmen kopieras utan omkodning och det mixade ljudbeskrivningsspåret läggs till. Om FFmpeg rapporterar ett kompatibilitetsfel för behållaren eller paketskrivningen när MP4 skapas, försöker Sonarpad automatiskt samma export som MKV, fortfarande utan att koda om videon. MKV kan också väljas uttryckligen. Förlängda pauser ignoreras i detta läge för att hålla ljud och video synkroniserade.

2. Förbättrad omanalys av segment i sparade syntolkningsprojekt. Fönstret använder nu AI-åtkomsten som redan är konfigurerad i huvudfönstret Skapa syntolkning, så API-nyckel, Sonarpad AI-saldo och modellval dupliceras inte längre. Ett omanalyserat segment behåller nu hela gruppen av projektbeskrivningar som hör till samma analysdel i stället för bara den första eller närmaste beskrivningen; alla beskrivningar i gruppen markeras som omanalyserade och ”Tillämpa segment och exportera MP3 igen” tillämpar hela gruppen på en gång. Förhandslyssning av utökade pauser syntetiseras nu direkt i stället för att söka i den exporterade MP3-filen, vilket förhindrar att förhandslyssningen börjar med filmljud eller klipper beskrivningen.

3. Den försiktiga reservvägen för Gemini-media har förbättrats utan att ändra det befintliga fungerande flödet. HTTP 400 `INVALID_ARGUMENT` fortsätter att använda mindre MKV-segment (cirka 15 MB) och därefter kompatibla MP4-segment. Om Gemini accepterar ett uppladdat segment men bearbetningen slutar i `FAILED`, laddar Sonarpad upp exakt samma segment en gång till och går först därefter över till MP4; serverns Code 13-försök begränsas till tre innan samma reservväg används, så att väntan inte kan bli oändlig. Nätverks-, kvot-, autentiserings- och syntesfel aktiverar inte denna väg.

4. Åtgärdade lagringen av AI-uppgifter när åtkomstläge byts. Den personliga Gemini-API-nyckeln och Sonarpad AI-åtkomstkoden sparas nu oberoende av varandra: byte mellan personlig API-nyckel och Sonarpad AI raderar inte längre den inaktiva uppgiften. Gemini-nyckeln sparas också när fältet tappar fokus.

5. En riktig Avbryt-knapp har lagts till för ”Analysera segment igen” och ”Tillämpa segment och exportera MP3 igen”. Förloppsindikatorn förblir aktiv och Avbryt stoppar nu FFmpeg-förberedelsen av segmentet, AI-processen, TTS-kontrollen och den slutliga exporten så snart det aktuella steget kan stoppas säkert. Tillämpning av ett omanalyserat segment är nu transaktionell: det befintliga projektet och mediefilen ersätts först efter en lyckad ny export; vid avbrott lämnas de tidigare filerna oförändrade och förslaget från omanalyseringen finns kvar för ett nytt försök.

6. Segmentomanalysen har korrigerats för beskrivningar utanför den första Gemini-chunken. Den isolerade chunken skickas nu till den befintliga workern med en lokal tidslinje som börjar på 0 och mappas sedan tillbaka till sin absoluta position i projektet, vilket undviker `Invalid prepared chunk timeline at chunk 1` utan att ändra den normala ljudbeskrivningskedjan.

7. Segmentanalysen i sparade projekt har gjorts om så att den använder samma säkerhetskontroller som en fullständig skapning av syntolkning. Det valda segmentet går nu igenom samma mono 16 kHz-ljudextraktion och Pyannote-analys av dialog/tystnad, samma tidsregler och Gemini-reservvägar, samma verkliga TTS-syntes med projektets röst och samma slutliga schemaläggare för säker placering och förlängda pauser. När segmentet tillämpas behåller Sonarpad de nya verifierade absoluta tiderna och uppdaterar Pyannote-skyddet för segmentet; den tidigare särskilda förhandsvisningslösningen för för lång omanalys-text har tagits bort.

8. Den strukturella ersättningen av ett segment efter fullständig omanalys har rättats. Sonarpad kräver inte längre att det nykontrollerade segmentet innehåller exakt samma antal beskrivningar som det sparade projektet: de gamla beskrivningarna i chunket ersätts av hela det säkra resultatet från den fullständiga pipelinen, så ett segment med 10 beskrivningar kan korrekt bli 9 (eller 11). Projektredigeraren visar omedelbart det nya antalet och de nya tiderna för förhandslyssning, medan det sparade projektet och mediefilen lämnas orörda tills segmentet har tillämpats och MP3-exporten slutförts.

9. Segmentomstartanalysen har byggts om så att det valda fysiska avsnittet behandlas som en riktig fristående minifilm. Sonarpad skickar nu minifilmen direkt till exakt samma `create_audio_description`-funktion som används vid normal fullständig skapande, så video och ljudspår delar samma lokala tidslinje och de vanliga kontrollerna för Pyannote, Gemini, verklig TTS, schemaläggning och säkerhet körs utan en separat omanalysimplementation. Först när den normala pipelinen är klar flyttas de lokala tiderna tillbaka till den ursprungliga positionen i filmen.

Textredigering
1. ”Slå ihop brutna rader” under Redigera > Text har förbättrats. Utan markering bearbetas hela dokumentet, med markering endast de markerade raderna. Funktionen bygger nu upp hela stycken genom att slå ihop efterföljande rader även när en mening redan slutar med skiljetecken; tomma rader fortsätter att skilja stycken åt och numrerade listor samt punktlistor bevaras.

10. Fixed segment reanalysis timing when mapping an independently analyzed mini-film back to the original movie. Sonarpad now applies the same small chunk-duration reconciliation used by the normal full analysis to descriptions, visual-evidence timestamps, and protected dialogue intervals, preventing progressive sub-second drift near the end of a chunk from moving narration onto speech.

Version 0.9.6 – 2026-09-09

1. Åtgärdat låsningar med vissa SAPI4-röster när markörföljning är aktiverad. Bryggan väntar nu tills syntesen verkligen är klar, med markörföljningen fortsatt aktiv.

2. Åtgärdat synkronisering mellan mikrofon och systemljud samt klick orsakade av avrundade tidsstämplar vid poddinspelning. Korrigeringarna gäller även när ljudkällorna sparas separat.

3. SAPI5-syntes för syntolkning körs nu i separata processer. Vid fel görs nya försök med färre samtidiga processer, färdiga beskrivningar behålls och den fungerande gränsen sparas för rösten.

Version 0.9.5 – 2026-09-07

AI-ljudbeskrivning
1. Förbättrad robusthet när Gemini tillfälligt returnerar `MALFORMED_RESPONSE` utan användbart innehåll. Sonarpad försöker nu exakt samma segment igen upp till tre gånger, med en kort väntan mellan försöken, i stället för att omedelbart avbryta hela ljudbeskrivningen. Normala Gemini-svar, säkerhetsblockeringar, tokenbegränsningsfel och det befintliga checkpoint-beteendet förblir oförändrade.

2. Ytterligare ett sällsynt fall av `invalid Gemini chunk timeline` har rättats för vissa videor där segment som förberetts av FFmpeg rapporterar en liten ackumulerad avvikelse i längd. Sonarpad lämnar den normala tidslinjehanteringen oförändrad när den är giltig och använder en försiktig justering endast när den normala tidslinjen annars skulle misslyckas och den totala avvikelsen är liten; stora eller misstänkta avvikelser ger fortfarande fel.

3. Den valfria Sonarpad AI-tjänsten för AI-ljudbeskrivning har lagts till. Användare kan fortsätta använda sin egen Gemini API-nyckel eller välja Sonarpad-tjänsten och ange en personlig Sonarpad-kod. Koden skyddas med Windows DPAPI. I tjänsteläget laddas förberedda videosegment upp direkt från användarens dator till Google via en tillfällig Google-uppladdningsadress; sonarpad.com tar inte emot eller lagrar videodata. Backend-tjänsten auktoriserar endast åtkomst, styr Gemini-modell/Standard-nivå och användningsgränser, registrerar förbrukning och returnerar Gemini-svaret.

4. Förbättrad redigering av sparade syntolkningsprojekt. Nu kan flera beskrivningar ändras efter varandra utan att varje ändring måste tillämpas direkt. Ändringarna ligger kvar i minnet; när “Tillämpa text” trycks kontrollerar Sonarpad varje ändrad beskrivning mot dess sparade tidsintervall och sparar sedan alla giltiga ändringar tillsammans. Om en beskrivning inte ryms i sitt intervall flyttas fokus tillbaka till den utan att övriga väntande ändringar går förlorade.

5. “Justera röst” har lagts till direkt före “Skapa syntolkning”. Det nya fönstret innehåller val för talsyntesmotor, röst, hastighet och volym samt “Testa röst”. När OK trycks återgår fokus exakt till “Justera röst” och de valda värdena används för syntolkningen som ska skapas. Om fönstret aldrig används förblir syntesen exakt som tidigare.


6. Röstkontrollerna i sparade syntolkningsprojekt har utökats. I Redigera projekt finns nu även hastighet, volym och ”Testa röst” utöver motor och röst. När röstinställningarna tillämpas kontrollerar Sonarpad alla befintliga beskrivningar igen med de nya syntesparametrarna och bygger bara om MP3-filen om alla fortfarande ryms inom sina sparade tidsintervall. Dialogfria intervall, Visual Evidence-tider, Gemini-beskrivningar och projektets tidsanalys återanvänds utan att AI-analysen körs igen.

Version 0.9.4 – 2026-09-04

AI-ljudbeskrivning
1. Åtgärdade ett problem med Matroska/WebM-videor som innehåller en ovanligt hög intern starttidsstämpel och som kunde stoppa skapandet av ljudbeskrivningen med felet ”invalid Gemini chunk timeline”. Sonarpad normaliserar nu källfilens och Gemini-segmentens längd endast när detta avvikande tidsstämpelmönster upptäcks, medan vanliga videor och det redan fungerande ljudbeskrivningsbeteendet förblir oförändrade.
2. Duckingen för ljudbeskrivningen har förbättrats för en mer naturlig mix. Originalljudet sänks nu mjukt redan innan rösten börjar och återgår mer gradvis till normal nivå efter beskrivningen; beskrivningar som ligger mycket nära varandra behåller en stabil bakgrundsnivå så att ljudet inte höjs och sänks upprepade gånger.
3. Åtgärdade ett problem som kunde göra att den slutliga exporten av ljudbeskrivningen misslyckades för vissa AVI-filer och andra källor med 5.1-/flerkanalsljud när avkodningen slutade med en ofullständig interfolierad ljudram. Sonarpad fyller nu säkert ut endast den sista ofullständiga ramen med exakt så många tysta sampel som behövs; redan kompletta ramar och vanliga mono-, stereo- eller flerkanalsfiler förblir oförändrade.


Version 0.9.3 – 2026-09-03

SAPI5-röster
1. Åtgärdade ett problem där vissa lokala SAPI5-röster kunde vara tysta när markörförflyttning var aktiverad, vid uppläsning med flera röster eller när ljudböcker/MP3-filer skapades. Sonarpad använder nu en SAPI5-filsynthesväg som fungerar tillförlitligt i Windows och fortfarande kan avbrytas under syntesen, medan vanlig direkt uppspelning är oförändrad.
2. Markörens position vid dialogläsning med flera röster har korrigerats. Rösttaggar som Sonarpad automatiskt lägger in för dialog behandlas nu endast som uppspelningsmetadata och inte som tecken i redigeraren, så markören hoppar inte längre framför den verkliga texten efter F4 eller F6. Läsning med en enda röst och uttryckliga <voice>-taggar i dokument förblir oförändrade.

AI-ljudbeskrivning
1. Kryssrutan ”Visa API-nyckel” har lagts till direkt efter fältet för Gemini API-nyckeln. Den är avstängd som standard; när den aktiveras visas hela nyckeln tillfälligt så att det går att kontrollera att den klistrades in komplett. När fönstret öppnas igen är nyckeln alltid dold.

Podcaster och Wikipedia
1. Efter att en podcastinspelning har sparats frågar Sonarpad nu om mappen som innehåller den sparade filen ska öppnas, på samma sätt som efter sparning av YouTube-/strömmande media.
2. När ytterligare en Wikipedia-artikel importeras till en redigerare som redan innehåller text läggs den nya artikeln nu till sist i stället för att infogas överst. Markören placeras i början av den nyimporterade artikeln.


Version 0.9.2 – 2026-09-02

AI-syntolkning
1. Åtgärdade ett problem som kunde göra att AI-syntolkning misslyckades vid den slutliga MP3-exporten för videor med flerkanalsljud, till exempel 5.1. Sonarpad mixar nu automatiskt ned flerkanalsljud till stereo endast när det krävs för MP3-kodning, utan att ändra mono- eller stereoexporter.
2. När AI-ljudbeskrivning startas för en video med flera ljudspår frågar Sonarpad nu vilket spår som ska användas innan bearbetningen börjar. Den tillgängliga kombinationsrutan kan ändras med piltangenterna; OK startar ljudbeskrivningen med det valda spåret, medan Avbryt stänger ljudbeskrivningsfönstret och återför fokus till Sonarpad-redigeraren.

YouTube och strömning
1. Åtgärdade ett problem där start av AI-syntolkning för en video på sida 2 eller senare i en YouTube-spellista eller kanal kunde öppna YouTube-valfönstret igen och ta fokus från syntolkningsfönstret. Sonarpad stänger nu väljaren korrekt utan att gå tillbaka till föregående sidor.
Version 0.9.1 – 2026-09-01

YouTube-nedladdningar
• Åtgärdade ett problem där förloppsfönster för YouTube-/strömmande nedladdningar upprepade gånger kunde hamna i förgrunden efter att användaren växlat till ett annat program med Alt+Tab. Nedladdningar fortsätter nu i bakgrunden utan att stjäla fokus.
• Förbättrade tillgängligheten för nedladdningsförloppet. När man återvänder till förloppsfönstret kan skärmläsare läsa aktuell status och procent. För spellistor meddelar Sonarpad även numret på det aktuella objektet, det totala antalet objekt och titeln.
• Åtgärdade falska watchdog-varningar om att programmet hängt sig under långa nedladdningar och konverteringar när förloppsfönstret fortfarande svarade.
• En kombinationsruta för Format har lagts till vid nedladdning av spellistor. Från videolistan kan du trycka Tab och välja MP4, MP3, M4A, OPUS, OGG, WAV eller FLAC innan flerfilsnedladdningen startas.
• Sparandet av strömmande media har organiserats om. Format och kvalitet väljs nu när materialet sparas i stället för i det första sökfönstret för strömning. ”Spara media” öppnar en gemensam dialog för Format och Kvalitet, och vid spellistenedladdning finns båda kombinationsrutorna.

AI-syntolkning
• Åtgärdade ett problem som kunde hindra AI-syntolkning från att starta med vissa MKV-videor. Sonarpad hanterar nu videor med oregelbundna eller saknade tidsstämplar på ett mer tillförlitligt sätt.

Version 0.9.0 – 2026-08-31

AI-syntolkning — stor ny funktion
• Lade till ”Skapa syntolkning med AI” under Verktyg > Multimedia. Sonarpad analyserar ljudet för att hitta utrymmen utan dialog, genererar beskrivningar med Gemini och använder de talsyntesmotorer som redan finns i Sonarpad samtidigt som talad dialog undviks.
• Förbättrade synkroniseringen mellan det som händer i videon och de genererade beskrivningarna, med automatiska kontroller av tidsangivelser som skapats av Gemini.
• ”Aktivera utökade pauser” är avstängt som standard. Det kan aktiveras för innehåll med mycket dialog eller lite tillgängligt utrymme så att längre beskrivningar ändå kan infogas.
• Sonarpad kan försöka känna igen personer och använda deras namn. Personkataloger kan behållas mellan avsnitt i en serie för bättre kontinuitet.
• Projekt kan sparas, redigeras senare och exporteras igen utan att allt behöver genereras på nytt med Gemini.
• Om processen avbryts behåller Sonarpad framstegen och kan fortsätta syntolkningen. Om Gemini-kvoten är slut kan du vänta, byta modell eller stoppa utan att förlora redan slutfört arbete.
• I fönstret kan du välja språk, detaljnivå, Gemini-modell, talsyntesmotor och röst, och de valda inställningarna koms ihåg.
• Modulen finns på alla 17 Sonarpad-språk. Under genereringen visar gränssnittet endast förlopp, aktuell status och Avbryt; när processen är klar kan MP3-filen öppnas direkt i den interna spelaren.

E-böcker och dokument
• Lade till import av DRM-fria Kindle-filer i formaten MOBI, AZW och AZW3, med text och kapitel tillgängliga i redigeraren och dokumentindexet.
• Lade till stöd för DAISY 2.02 och DAISY 3. DAISY-ljudböcker använder Sonarpads interna spelare och följer kapitelnavigering och uppspelningsgränser.
• Kindle- och DAISY-filer importeras utan att originalfilen skrivs över; DRM-skyddade Kindle-böcker avvisas uttryckligen.
• Åtgärdade EPUB ”Spara som”: när TXT eller ett annat format väljs används nu den valda filändelsen och den ursprungliga EPUB-filen förblir kopplad till det öppna dokumentet.

RSS och artiklar
• Lade till flerval för RSS-artiklar så att flera artiklar kan tas bort i en enda åtgärd.
• RSS stöder nu riktiga mappar som bevaras vid import och export av OPML, även tomma mappar.
• Flöden kan ordnas om i den aktuella mappen med Flytta upp, Flytta ned, Flytta längst upp, Flytta längst ned och Flytta till position.

Tillgänglighet, guider och gränssnitt
• Sonarpads guider har organiserats om med ett index, och en fullständig guide till AI-syntolkning har lagts till.
• Åtgärdade ett problem i den tyska översättningen som kunde hindra dialogrutorna Öppna, Spara som och andra filvalsdialoger från att visas.

Röster och språk
• Den nedladdningsbara Google TTS-katalogen har vuxit från 104 till 156 paket och från 53 till 81 språkvarianter.
• Lade till nya Google TTS-paket och lokaliserade namn för ytterligare språk i gränssnittet.

Version 0.8.4 – 2026-07-24

Redigering av EPUB-dokument
• Sonarpad kan nu inte bara öppna EPUB-dokument utan även redigera dem och spara dem igen i EPUB-format samtidigt som ursprunglig formatering, innehållsförteckning, fotnoter, bilder, formatmallar, metadata och interna länkar bevaras.
• EPUB finns i ”Spara som” för dokument som öppnats från en EPUB-fil. Vid sparning uppdateras endast den ändrade texten och bokens struktur behålls intakt.

Tillförlitlighet för ljudböcker
• Åtgärdade ett sporadiskt problem där en syntesenhet efter fem misslyckade Google TTS-försök kunde kasseras tyst, vilket kunde göra att den färdiga ljudboken saknade en del av texten.
• Google-enheter försöks nu igen tills de lyckas eller användaren avbryter. Starten av arbetsprocesser förskjuts för att minska tillfälliga konflikter med Chrome och filer, och Sonarpad stoppar nu i stället för att spara en ljudbok som saknar ett segment.
• Edge-ljudböcker försöker nu på nytt vid tillfälliga nätverks-, WebSocket-, timeout-, tjänstegräns- och ogiltigt-ljud-svar tills det lyckas eller användaren avbryter, även med blandade röster och tidsbaserad uppdelning. SAPI4 och SAPI5 behåller adaptiv begränsad återhämtning; om ett segment ändå misslyckas stoppar Sonarpad utan att spara en ofullständig ljudbok.

Navigering i digitala bibliotek
• Sökresultat i LibriVox, Internet Archive och Project Gutenberg använder nu sidnavigering som YouTube: ”Gå till föregående resultat” visas överst och ”Gå till nästa resultat” längst ned.
• Åtgärdade fokusövergångar i LibriVox: när en bok eller ett kapitel öppnas flyttas NVDA-fokus inte längre till huvudredigeraren innan nästa lista eller spelare öppnas.
• Lade till ett fokusskydd i LibriVox under sökningar och inläsning av böcker: en lokaliserad laddningsdialog ligger kvar i förgrunden medan begäran pågår och förhindrar att NVDA-fokus hoppar till Kommandotolken, Windows Terminal eller ett annat program.

Nedladdning av YouTube-spellistor
• Lade till ett tillgängligt kommando för flerval i YouTube-spellistor, så att användaren kan välja vilka videor som ska laddas ned utan att ändra det befintliga kommandot ”Spara media” för objektet som spelas upp.
• Valda objekt laddas ned ett i taget med det format och den kvalitet som valdes när spellistan öppnades, får numrerade filnamn som bevarar spellistans ordning och sparas i en särskild mapp i den konfigurerade Media-mappen.
• Urvalsfönstret innehåller kommandona Markera alla och Avmarkera alla, meddelar antalet valda objekt, stöder avbrytning samtidigt som färdiga filer behålls och rapporterar objekt som inte kunde laddas ned.
• Poster i spellistor är nu inbyggda kryssrutor: skärmläsare meddelar automatiskt varje titel, kontrollroll och markeringsstatus utan att lägga till urvalsord i den synliga titeln eller använda framtvingat tal.

Version 0.8.3 – 2026-07-23

Mörkt läge
• Lade till ett mörkt läge som kan aktiveras från menyn Visa och sparas i användarinställningarna.
• Det mörka temat används i redigeraren, menyer, sekundära fönster och huvudkontroller, med textfärger anpassade för att bevara läsbarhet och tillgänglighet.

Tyska
• Lade till tyska som ett komplett gränssnittsspråk som kan väljas i Alternativ.
• Nyheter och RSS, stavningskontrollen, kalendern samt alla citat, donationer, guiden och ändringsloggen finns nu helt på tyska.

Brasiliansk portugisiska och Google Nyheter
• Lade till brasiliansk portugisiska som ett komplett gränssnittsspråk, separat från portugisiska (Portugal) och valbart i Alternativ.
• Hela gränssnittet, kalenderposter och citat, stavningskontrollen, donationer, guiden och ändringsloggen finns på brasiliansk portugisiska.
• Google Nyheter stöder nu brasiliansk lokalisering, brasilianska kategorier och separata standardkällor för brasilianska RSS-flöden.
• Relaterade Google Nyheter-källor för samma nyhet visas som tillgängliga underobjekt i trädet när flödet tillhandahåller dem.

LibriVox
• Optimerade LibriVox-sökningar för att undvika alltför många förfrågningar till tjänsten och låsningar i gränssnittet. Stora kataloggenomsökningar togs bort, antalet försök minskades och kortare tidsgränser infördes.

Talsyntes
• Sekvenser med tre eller fler punkter normaliseras nu före uppläsning, vilket förhindrar att vissa röster säger ”punkt punkt” eller skapar segment som bara består av skiljetecken.

Relaterade artiklar i Google Nyheter
• För varje nyhet visas nu relaterade artiklar när sådana finns, det vill säga andra artiklar som handlar om samma nyhet. För att läsa dem expanderar du helt enkelt huvudartikeln när Sonarpad meddelar att relaterade artiklar finns. Den som inte vill expandera avsnittet kan bara trycka Enter på huvudartikeln och läsa nyheten som vanligt.
• Relaterade artiklar använder nu samma system för läst/oläst som huvudartiklar, inklusive tillgängliga meddelanden, datum och tid, sparad status samt bevarande efter uppdatering av flöden eller omstart av Sonarpad.

Meddelanden för ljudboksdelar
• Lade till kombinationsrutan ”Meddelande i början av varje del” i Ljudalternativ. För ljudböcker som delas upp i flera filer kan varje del börja utan meddelande, med boktiteln, titeln och delnumret, filnamnet eller filnamnet och delnumret.

Version 0.8.2 – 2026-07-17

Digitala bibliotek och ljudböcker
• Lade till Project Gutenberg, med sökning efter titel eller författare och språkval.
• EPUB-böcker från Project Gutenberg laddas ned till Documents\Sonarpad\Documents; när nedladdningen är klar frågar Sonarpad om boken ska öppnas direkt i redigeraren.
• Lade till Internet Archive för att söka efter och lyssna på ljudsamlingar, inklusive äldre radioprogram, tal och livemusik.
• Lade till LibriVox för att söka efter ljudböcker efter titel eller författare och spela kapitel direkt med samma spelare som används för poddar.
• De tre nya funktionerna finns i menyn Verktyg och, när menygruppering är aktiverad, i avsnittet Läsning.

Transkribering av långt ljud
• Åtgärdade transkribering av långa ljudfiler: ljudet delas nu automatiskt i delar om 15 minuter, transkriberas en del i taget och sammanfogas sedan igen, vilket förhindrar fel som kunde uppstå med långa inspelningar.

YouTube
• De mest användbara åtgärderna som tidigare bara var tillgängliga efter att en YouTube-video öppnats och menyn Uppspelning valts finns nu även direkt i samma videos snabbmeny, till exempel ”Transkribera aktuellt ljud”, ”Skapa syntolkning med AI” och ”Spara media”.
• Lade till ”Kopiera länk”, även tillgängligt med Ctrl+C, för att kopiera URL-adressen till den valda YouTube-videon, spellistan eller kanalen till Urklipp.

Version 0.8.1 – 2026-07-16

Google text-till-tal
• Åtgärdade starten av Google TTS på Windows-system där anslutningar som accepterades av den interna webbläsarservern ärvde icke-blockerande socketläge, vilket orsakade fel 10035 och hindrade nedladdade röster från att tala.
• Sonarpad väntar nu tills Chrome- eller Edge-WASM-motorn är helt inläst före röstförhandsgranskning eller läsning med F5, vilket förhindrar felet ”Chrome WASM TTS engine was not loaded”.
• Den dolda webbläsaren inaktiverar sidöversättning och tillgänglighet i renderaren så att den inte kan meddela ”Översätt sida” eller störa läskommandon.
• Panelen ”Röster i redigeraren” visar nu knappen ”Hantera Google-röster...” när Google-motorn är vald och uppdaterar listan över installerade röster direkt när hanteraren stängs.
• Varningar om beroenden som visas när Google-röstpaket tas bort är nu lokaliserade på alla gränssnittsspråk.

Uppdateringsupplevelse
• Efter en automatisk uppdatering öppnas nu slutförande- och ändringsloggsfönstret efter den första återställningen av fokus till redigeraren och ligger kvar i förgrunden i stället för att visas först efter att Tab trycks.

PDF-dokument
• Åtgärdade PDF-filer vars inbäddade text innehöll NUL-tecken och kapades vid den första förekomsten när den lästes in i redigeraren.
• När pdf-extract returnerar inbäddade NUL-tecken försöker Sonarpad nu igen med PDFium. Återstående NUL-tecken tas bort innan text skickas till Windows-kontroller, så resten av dokumentet bevaras.

Menytillgänglighet
• Tog bort generering av mnemonics under körning: snabbtangenter skrivs nu uttryckligen i var och en av de 15 gränssnittsöversättningarna och förblir därför identiska mellan starter.
• Granskade alla stabila objekt och undermenyer i huvudmenyn, inklusive Uppspelning, teckensnittsval, Spara bild och Visa EPUB-index. Saknade eller dubblerade mnemonics bland syskon korrigerades direkt i översättningarna.
• Automatiska tester validerar nu bara översättningarna och misslyckas om en mnemonic saknas, är ogiltig eller duplicerad; de ändrar aldrig menytexter under körning.
• I ovanligt stora menyer där de översatta etiketterna inte ger tillräckligt många unika tecken visas en uttrycklig numerisk snabbtangent med Windows standardformat ”(&1)”.

Version 0.8.0 – 2026-07-15

Onlineordbok
• Lade till tyska i Wiktionary-ordboken online.
• Tyska definitioner och synonymer tolkas nu med strukturen i tyska Wiktionary, i stället för att bara lägga till språket i urvalslistan.

Tillförlitlighet för SAPI5-ljudböcker
• Skapande av SAPI5-ljudböcker behåller upp till 12 parallella arbetsprocesser när den valda rösten ger tillförlitligt resultat.
• Varje genererad del kontrolleras nu med filstorlek, uppskattad längd och en försiktig jämförelse med den tilldelade texten.
• Saknade eller misstänkta delar genereras automatiskt om med gradvis lägre samtidighet: 12, 8, 6, 4, 2 och slutligen 1 arbetsprocess. Endast problematiska delar upprepas.
• Den tillförlitliga gränsen för arbetsprocesser koms ihåg separat för varje SAPI5-röst, utan att sakta ned röster som fungerar korrekt med 12 arbetsprocesser.
• En slutlig integritetskontroll hindrar Sonarpad från att tyst acceptera en MP3 som är mycket kortare än de genererade delarna.
• Detaljerad diagnostik skrivs till `sapi5_audiobook_diagnostic.log`.
• Varje SAPI5-syntesenhet körs nu i en separat dold Sonarpad-process. Om en röst från tredje part kraschar stängs bara den arbetsprocessen och huvudprogrammet förblir öppet.
• Under samma skapande av ljudbok försöks ofärdiga delar omedelbart igen med nästa lägre nivå av samtidighet; redan validerade delar bevaras.
• Återställning vid nästa start finns kvar som ett extra skydd endast om huvudprogrammet eller datorn avbryts.

Arbetsprocesser för SAPI4-ljudböcker
• Antalet SAPI4-processer som användaren väljer respekteras nu, upp till ett tekniskt maximum på 64; den tidigare dolda gränsen på 16 har tagits bort.
• Det effektiva antalet minskas endast när ljudboken innehåller färre arbetsenheter än begärt.
• Om en eller flera SAPI4-bryggprocesser misslyckas bevaras färdiga delar och endast misslyckade enheter försöks automatiskt igen med gradvis lägre samtidighet.
• Sonarpad kontrollerar nu SAPI4-bryggans avslutningsstatus och avvisar tomma eller ogiltiga ljuddelar i stället för att behandla dem som lyckade.

Proxykonfiguration
• Lade till ett separat fält för proxyporten i nätverksinställningarna.
• Porten kan nu anges oberoende av proxyadressen, valideras från 1 till 65535 och ersätter korrekt en port som redan finns i URL-adressen.

Radiosökning efter språk och land
• Filtren Språk och Land uppdateras nu med alla tillgängliga poster från Radio Browser-katalogen i stället för att begränsas till en fast lista.
• Språknamn känns nu igen även när Radio Browser tillhandahåller dem i ett annat skriftsystem, som inhemska namn, förkortningar eller kombinationer av flera språk, och visas översatta till det aktuella gränssnittsspråket. Värden som inte är riktiga språk, till exempel siffror, genrer, länder eller generiska etiketter, filtreras bort.
• Katalogen uppdateras i bakgrunden, med en reservlista som fortfarande kan användas när Radio Browser inte kan nås.
• Dubbletter av Radio Browser-språkposter som blir identiska efter översättning slås nu ihop till ett enda objekt i kombinationsrutan, vilket förhindrar tysta steg med skärmläsare.

Stor förbättring: synkronisering mellan tal och markörrörelse
• Synkroniseringen mellan taluppspelning och markörrörelse har förbättrats avsevärt för alla talsyntesmotorer som stöds.
• När ”Flytta markören under läsning” är aktiverat använder Sonarpad nu ett gemensamt förloppssystem för Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 och OneCore.
• Markören följer den text som faktiskt läses upp mer exakt, med jämnare uppdelning av meningar och fraser.
• För tidiga förflyttningar, fördröjningar, oregelbundna hopp och skillnader mellan talsyntesmotorer har minskat kraftigt.
• Rätt position bevaras nu mer tillförlitligt efter paus, återupptagning, sökning i ett dokument eller byte av talsyntesmotor.

Separata spår vid poddinspelning
• Lade till ”Spara mikrofon och system- eller programljud i separata filer”.
• När mikrofonen och en annan källa spelas in tillsammans kan Sonarpad skapa en fil med endast mikrofonen och en andra fil med systemljud, ett program eller de valda programmen.
• Separat inspelning av källor finns både för MP3 och WAV.
• När alternativet är inaktiverat fortsätter Sonarpad att skapa en normalt mixad fil.
• Separata filer gör det enklare att justera volym, ta bort brus och senare redigera poddar, intervjuer och handledningar.

Schemalagda radioinspelningar
• Radioinspelningar kan nu schemaläggas i förväg.
• För varje inspelning kan användaren välja station, dag, starttimme och minut samt längd.
• En anpassad längd från 1 till 1 440 minuter finns tillgänglig.
• Inspelningar kan köras en gång, varje dag eller varje vecka.
• Inspelningsfönstret visar nu aktiva och schemalagda inspelningar, planerat datum och tid, längd och återstående tid före start tydligare.
• Schemalagda inspelningar kan använda Windows Aktivitetsschemaläggare, så att de kan starta automatiskt även när Sonarpad inte redan är öppet.

Kalender
• Lade till en komplett kalender som kan användas med tangentbordet.
• Användare kan bläddra mellan föregående och följande dagar, snabbt återgå till idag och kontrollera helgdagar och bemärkelsedagar.
• Lade till dagens helgon och dagens citat, som kan läsas, läsas upp eller kopieras.
• Påminnelser kan skapas, redigeras, tas bort, skjutas upp och markeras som slutförda.
• Aviseringar kan visas exakt vid angiven tid eller i förväg och kan använda Windows schemaläggning även när Sonarpad är stängt.

Väder
• Lade till ett avsnitt för väderprognoser.
• Användare kan söka efter en stad och snabbt öppna nyligen visade platser igen.
• Aktuella förhållanden, temperatur, minimi- och maximivärden, luftfuktighet, nederbördsrisk och prognoser för följande dagar finns tillgängliga.
• Temperaturen kan visas i Celsius, Fahrenheit eller väljas automatiskt.

Filmer på bio
• Lade till ett avsnitt för filmer som går på bio och kommande premiärer.
• Sökning efter titel, handling, premiärdatum och uppspelning av trailer finns tillgängligt.

Google text-till-tal
• Lade till Google TTS för dokumentläsning och skapande av ljudböcker.
• Lade till en rösthanterare för att lista röster, filtrera efter språk, ladda ned dem och ta bort röster som inte längre behövs.
• Hastighet, volym och tonhöjd kan justeras.
• Tonhöjden för Google Natural-röster tillämpas direkt av motorn för ett naturligare och stabilare resultat.
• Svarstiden och tillförlitligheten för Google TTS har förbättrats, med tidsgränser för syntes anpassade till vald talhastighet.
• Onödig väntan när motorn inte svarar har minskat och hanteringen av fel och avbrott har förbättrats.
• Diagnostikloggningen är stabilare vid samtidiga åtgärder.

EPUB-innehållsförteckning
• Sonarpad känner nu igen innehållsförteckningen som är inbäddad i EPUB-böcker.
• Dess närvaro meddelas och den kan öppnas från menyn Visa.
• Kapitel och underkapitel visas hierarkiskt.
• Om Enter trycks flyttas man omedelbart till den valda platsen i boken.

Nyheter och RSS-källor
• Utökade avsnittet Nyheter med nya verktyg för sökning och organisering.
• Lade till val av språk för nyheter.
• Användare kan söka bland RSS-källor och läsa nyheter från sin stad.
• RSS-källor från gemenskapen kan bläddras, läggas till i den personliga samlingen och skickas in till Sonarpad-gemenskapen.

Poddinspelning
• Användare kan spela in endast mikrofonen, allt systemljud, ett program, flera valda program eller mikrofonen och programmen tillsammans.
• Mikrofonenhet och ljudkälla kan väljas, källornas volym kan justeras separat och nivåerna kan övervakas i realtid.
• Lade till paus och återupptagning, MP3- eller WAV-utdata, val av MP3-bithastighet och val av målmapp.
• Datorn kan hållas vaken under inspelningen.
• Separata filer får olika namn så att mikrofonspåret omedelbart kan skiljas från system- eller programljud.

Radio
• Radioavsnittet har omorganiserats i stor omfattning.
• Stationer kan sökas efter namn eller fritext, språk, land, stad, musikgenre eller kategori.
• Hanteringen av favoriter har förbättrats och alla filter kan snabbt återställas.
• Stationer kan skickas in till Sonarpad-gemenskapen.
• Lade till liveinspelning, ”Spela in och spela”, en lista över inspelningar samt borttagning och hantering av inspelningar.
• Radioinspelningar lagras i en egen mapp i huvudmappen för inspelningar.

Medieuppspelning
• Förbättrade mediespelarens stabilitet avsevärt.
• Åtgärdade ett problem som kunde blockera mpv och gjorde kommunikationen med spelaren mer tillförlitlig.
• Förbättrade öppning av olika typer av mediefiler.
• Sonarpad kommer nu ihåg volymen som användes vid uppspelning.
• Hanteringen av strömmar och inspelningar har förbättrats.
• Åtgärdade filer som öppnas från Windows med dubbelklick eller ”Öppna med”.

PDF-dokument
• Lade till identifiering av formulärfält i PDF-dokument.
• Sonarpad kan hitta ifyllbara fält, presentera dem i en tillgänglig textform, låta användaren redigera värdena och spara de angivna uppgifterna tillbaka till PDF-filen.
• Åtgärdade beräkning av markörposition under tal, särskilt i dokument med flerbytes-tecken eller komplexa strukturer.
• Det nya gemensamma synkroniseringssystemet förbättrar ytterligare markörrörelsen med alla talsyntesmotorer.

Tillgänglighet och tangentbordskommandon
• Förbättrade vanliga redigeringskommandon i hela programmet.
• Kopiera, klipp ut, klistra in, markera allt, ångra och gör om skickas nu korrekt till det fält som har fokus, även i sekundära fönster och dialogrutor.
• Åtgärdade ett problem som kunde hindra punktskriftsskärmar från att uppdateras korrekt.
• Förbättrade fokushanteringen i sekundära fönster.
• Åtgärdade språkvalet i Wikipedia-fönstret.
• Lade till ett alternativ för att gruppera funktionerna i Verktyg-menyn efter kategori.
• Lade till konfigurerbara åtgärder för att snabbt öppna Kalender, Väder och Filmer på bio.
• Förbättrade presentationen av ändringsloggen efter en uppdatering.

Ljudböcker
• Förbättrade skapande av ljudböcker när dialogrutor eller andra modala fönster är öppna.
• Förloppshanteringen är robustare och ignorerar inaktuella ljuduppdateringar, vilket minskar låsningar, felaktiga meddelanden och fönster som inte svarar.
• Google TTS kan också användas för att skapa ljudböcker med kontroller för hastighet, volym och tonhöjd.

Artificiell intelligens
• Uppdaterade standardmodellen för Gemini till `gemini-3.5-flash`.

Allmänna korrigeringar
• Åtgärdade flera låsningar i mpv-uppspelning.
• Åtgärdade öppning av vissa ljud- och videofiler.
• Förbättrade kommandon som skickas till mediespelaren.
• Åtgärdade återställning av markör under taluppspelning.
• Åtgärdade kortkommandon i textfält i hjälpfönster.
• Förbättrade stabiliteten vid skapande av ljudböcker.
• Åtgärdade filer som öppnas externt genom Windows.
• Förbättrade den övergripande hanteringen av media, RSS, radio och EPUB.

Version 0.7.1 – 2026-05-13

Nya funktioner och förbättringar
• Skapade den officiella webbplatsen sonarpad.com, en ny samlingspunkt för senaste nytt, nedladdning av den senaste programversionen, besökares kommentarer och i framtiden alla Sonarpad-poddar. Hjälp-menyn innehåller nu också ”Besök sonarpad.com” för snabb åtkomst till den officiella webbplatsen.
• Åtgärdade ett problem där filer med accenter eller specialtecken gav fel när rösttranskribering startades.
• Från och med nu visar objekt som Radbrytning och Visa video under uppspelning i Visa-menyn alltid rätt tillstånd, aktiverat eller inaktiverat.
• Förbättrade YouTube-sökning så att användaren kan gå tillbaka till föregående sida eller vy med Esc.
• Lade till en preliminär kontroll av om en video kan spelas. Uppspelningen har också förbättrats: Sonarpad kan nu spela videor eller spellistor märkta som mixar, vilket tidigare inte gick.
• Förbättrade automatisk hantering av bokmärken. Tidigare låg automatiska bokmärken kvar om alternativet först aktiverades och sedan stängdes av; nu ignoreras de korrekt tills funktionen aktiveras igen. När slutet av en mediefil nås tas bokmärket dessutom bort automatiskt.
• Förbättrade hanteringen av taggar när dialogrutor är aktiverade. Sonarpad hanterar nu båda funktionerna korrekt så att taggar kan infogas även när dialogalternativet är aktiverat.
• Förbättrade röstinställningarna genom att tydligt separera varje motor, vilket ger mer exakta justeringar. Röstprofiler behåller nu korrekt inställningarna för varje enskild motor: Edge, SAPI5 och SAPI4.
• Lade till en tagg för pauser, direkt från alternativen eller från röstpanelen genom att trycka Tab från redigeraren. Valen är 250 ms, 500 ms, 1 sekund, 2 sekunder eller en anpassad längd.
• Åtgärdade beteendet när en YouTube-video spelas och transkribering startas. När man återvänder med Alt+Tab ligger fokus nu korrekt på knappen Avbryt i den aktiva transkriberingen.
• Transkriberingar sparas nu automatiskt när processen är klar.
• Förbättrade import från Wikipedia. Du kan välja att bara läsa ett avsnitt och sedan återvända till sökningen från artikeln med Esc, eller importera hela artikeln. Du kan också välja vilket Wikipedia-språk som ska användas.
• Lade till ett globalt radioavsnitt där stationer kan sökas efter land, språk och genre. Lokala radiostationer kan också läggas till i Sonarpads databas så att andra användare kan lyssna på dem, och stationer kan läggas till som favoriter.
• Lade till ett ruttavsnitt för att beräkna resvägar med färdsätt: gång, cykel, bil eller rullstol. Du kan välja kortaste eller snabbaste väg och om passerade kommuner ska visas. När rutten importerats kan den visuella kartan också sparas via Arkiv > Spara bild.
• Lade till Skriv ut i Arkiv-menyn. Sonarpad skriver ut TXT-filer med sitt eget system och använder associerat program för andra filer, till exempel DOCX och PDF, för att bevara ursprunglig layout så långt som möjligt.
• Lade till en översättningstjänst för varje dokument, tillgänglig från redigerarens snabbmeny. Användare kan använda kostnadsfria DeepL och Google Translate utan API-nyckel; med en Gemini API-nyckel kan de i stället översätta med Gemini.
• I översättningsmenyn kan målspråk väljas. Menyn ordnar automatiskt om sig: om användaren först väljer engelska, sedan franska och därefter italienska visas dessa tre alternativ överst i språkmenuyn.
• Om användaren anger sin Gemini API-nyckel får den också tillgång till Sammanfatta text i snabbmenyn för att sammanfatta valfri artikel.
• Lade till en meny i Uppspelning som är synlig när en mediefil spelas och kan dela aktuell media. Den fungerar med MP3, MP4 och andra format och delar antingen efter antal delar eller längden på varje del.

Version 0.7.0 – 2026-04-25

Nyheter
• Lade till stöd för mpv-spelaren vid strömmande uppspelning. Videor från YouTube och stödda webbplatser spelas nu direkt; om användaren väljer att behålla dem laddas de ned som tidigare. Vid transkribering av strömmande innehåll laddas det först ned och transkriberas sedan. mpv används också för lokala videor och undertexter, vilket ger bättre kompatibilitet med många format som tidigare inte stöddes fullt ut.
• Förbättrade poddinspelning av systemljud: nu kan allt systemljud, ett enda program eller flera program samtidigt väljas. Detta är integrerat i normal inspelning så att mikrofonen fortfarande kan aktiveras eller inaktiveras separat.
• Lade till hindi. Gränssnittet översattes och RSS-flöden, ändringslogg och Sonarpad-guide lades till.
• Lade till ett alternativ på fliken Redigerare för att alltid flytta markören till början av raden med upp- och nedpilarna.
• Lade till M4B i menyn ”Konvertera ljud”.

Korrigeringar
• Åtgärdade `F10` så att den åter växlar till nästa favoritröst under textläsning.
• När en poddinspelning pågår stänger stängning av ett annat dokument inte längre den aktiva inspelningen.
• I YouTube-kommentarer som öppnas från ”Spela strömmande ljud...” läser Sonarpad först bara in de första 50 huvudkommentarerna, alltid med alla svar till dessa, och lägger till ett sista objekt för att vid behov läsa in alla kommentarer.
• Bokmärken visas och hanteras nu i positionsordning för både textdokument och mediefiler i stället för skapelseordning. Ett bokmärke på exakt samma position läggs inte längre till igen.
• Lade till ett alternativ i Bokmärken-menyn för automatisk bokmärkeshantering. När en lokal eller strömmande fil spelas och stängs skapar Sonarpad automatiskt ett bokmärke vid nådd position och fortsätter där nästa gång filen öppnas. Samma gäller textfiler: markörpositionen koms ihåg när filen stängs, och om läsning startas sparas den senast lästa meningen så att läsningen fortsätter exakt därifrån.
• Lade till ett alternativ i Visa-menyn för att visa video för lokala eller strömmande filer. Videon visas i ett förstorat fönster där alla kontroller döljs om inte Alt trycks eller musen förs mot fönstrets överkant. Detta ska göra innehållet större och mer användbart för personer med nedsatt syn.

Version 0.6.9 – 2026-04-08

Korrigeringar
• Förbättrade Sök i filer: vid Bläddra efter mapp går fokus direkt till mapplistan; Enter på ett resultat bryter inte längre tangentbordskommandon; Esc återgår till tidigare markerat resultat; och efter Alt+Tab går fokus antingen till sökfältet eller resultatlistan om resultat är öppna.
• F5 började alltid läsa från början. Det är nu åtgärdat så att läsningen börjar vid aktuell markörposition, medan `Shift+F5` och `Ctrl+F5` bevaras för föregående och nästa mening.
• Efter Gå till rad kunde Esc flytta fokus ut ur Sonarpad. Nu återgår fokus korrekt till redigeraren.
• Alternativet `Radbrytning` tillämpas nu omedelbart på redan öppna dokument i stället för först när filen öppnas igen.

Version 0.6.8 – 2026-04-07

Nyheter
• Lade till ett nytt objekt i Spela-menyn för att transkribera valfri ljud- eller videofil med Whisper. I Alternativ finns nu avsnittet ”AI och transkribering”, där modell kan väljas, valfritt CUDA-stöd för NVIDIA-grafikkort aktiveras, originalspråk bevaras och tidsstämplar slås på eller av.
• Lade till ett nytt kommando i Spela-menyn, `Transkribera aktuell mapp`, som transkriberar alla ljudfiler som stöds i mappen för den öppna mediefilen till ett enda sammanfogat dokument, med eget förlopp, status för aktuell fil och stöd för avbrytning. Det kan också startas med `Alt+Shift+C`.
• Lade till offline-röstdiktering med samma arbetsflöde som ljudtranskribering. Som standard startar `Ctrl+Shift+Space` diktering och samma kortkommando stoppar den; kortkommandot kan ändras i Alternativ. Från andra aktiveringen blir dikteringen snabbare eftersom motorn hålls redo i minnet; förladdning och återanvändning stängs automatiskt av på datorer med mindre än 4 GB RAM.
• Lade till ett nytt Redigerare-alternativ, avstängt som standard, som gör att `Esc` stänger redigerarfönstret.
• Poddsökning använder nu `iTunes + Spreaker` som standard, med dubblettfiltrering när samma podd hittas på båda plattformarna.
• Förbättrade bläddring och sökning i Apple Podcasts: poddsökning, kategorier och topplistor per kategori använder nu landet som valts för poddkatalogen. I Alternativ > RSS / Poddar kan Automatisk använda systemets land eller ett annat land väljas manuellt.
• Ökade resultatgränsen för Apple Podcasts-kategorier. Första öppningen visar fortfarande 50 resultat; med `Läs in fler resultat` laddar Sonarpad upp till totalt 200, Apples gräns, och låter användaren navigera vidare utan att gränssnittet blir tungt.
• Sonarpad finns nu även för Mac med en delmängd av funktionerna. Projektlänk: https://github.com/Ambro86/Sonarpad-Mac

Förbättringar
• Lade till fler än 50 valbara länder för poddkatalogen, så att betydligt fler nationella kataloger kan användas.
• ”Spela strömmande ljud...” kan nu också söka på YouTube från valfri textfråga eller ta emot en länk till en YouTube-kanal eller spellista och visa resultaten.
• Förbättrade resultatvisningen i ”Spela strömmande ljud...”: YouTube-poster visar nu titel, längd, kanal och antal visningar tydligare.
• ”Spela strömmande ljud...” stöder nu även YouTube-kommentarer: de kan öppnas från snabbmenyn, svar kan läsas och kommentarstrådar expanderas med högerpil.
• Lade till YouTube-favoriter för kanaler och spellistor i ”Spela strömmande ljud...”. De kan läggas till från resultatens snabbmeny, öppnas från Favoriter-listan som nås med Tab efter fältet för YouTube-URL/fråga och senare tas bort från samma lista. I YouTube-sökresultat finns snabbmenyn bara för kanaler och spellistor.
• ”Spela strömmande ljud...” kan nu be om inloggningsuppgifter när en strömningssajt kräver inloggning. De kan anges, sparas för sajten och senare hanteras i Alternativ > Ljud.
• Förbättrade fokushanteringen under ”Spela strömmande ljud...” så att förloppsfönstret är stabilare under nedladdning och konvertering.
• Lade till två nya navigeringsåtgärder i Röst-menyn: `Föregående mening` och `Nästa mening`, med konfigurerbara kortkommandon under textläsning.
• Standardkortkommandot för `Kör fil med tolk` är nu `Ctrl+Shift+F5`, så att `Shift+F5` kan användas för `Föregående mening`.
• Lade till röstprofiler i Alternativ > Röst. Profiler kan läggas till, byta namn och tas bort.
• Utökade alternativen för hur långt uppspelningen ska spolas tillbaka i Alternativ > Ljud, med värden från 1 sekund till 2 timmar.
• Lade till rysk översättning tack vare Dmitriy.
• Lade till ett alternativ i Alternativ > Ljud för namngivning av ljudboksdelar: `Titel + nummer`, `Endast nummer` eller `Nummer + titel`.
• Lade till favoritartiklar i RSS: artiklar kan läggas till i ett särskilt Favoriter-flöde från snabbmenyn.
• RSS-flödet Favoriter kan tas bort och skapas automatiskt igen när en ny artikel läggs till som favorit.
• Lade till RSS-kortkommandon för att flytta flöden upp/ned: `Ctrl+Shift+Upp` och `Ctrl+Shift+Ned`.
• Förbättrade RSS-fönstret med inbyggd förhandsvisning av artiklar, nåbar snabbt med Tab innan hela artikeln öppnas i redigeraren.
• Lade till den uttryckliga RSS-posten ”Läs in fler nyheter” i slutet av flöden när fler objekt finns. Enter läser in nästa grupp och flyttar fokus till den första nyinlästa artikeln.
• I röstordboken finns nu kryssrutan ”Matcha skiftläge” när en ersättning läggs till eller redigeras, så att varje ersättning kan följa eller ignorera skiftläge.

Felkorrigeringar
• ”Spela strömmande ljud...” respekterar nu poddcachens gräns i Alternativ, och samma gräns gäller även uppspelning av syntolkningar.
• Åtgärdade Wikipedia-import så att citatblock på sidor nu importeras korrekt.
• Förbättrade webbsideparsern för WordPress-sidor där listposter och vissa avsnittsrubriker kunde utelämnas.
• ”Gå till rad” förifyller nu fältet med aktuell rad.
• Åtgärdade OPML-export för poddar och RSS så att exporterade filer nu accepteras av iTunes.
• Lade till lokaliserade bekräftelser för korrekt OPML-import och -export av RSS-flöden och poddar.
• Åtgärdade ett fel där val av en YouTube-kanal från en textsökning i ”Spela strömmande ljud...” kunde få programmet att verka låst i stället för att öppna kanalens videor.
• Åtgärdade att listan över öppna filer visades i Hjälp-menyn i stället för Fönster-menyn.
• Åtgärdade ett strömningsfall där uppspelning kunde starta men dialogen ”Laddar ned ström” låg kvar när filen redan hade målformatet.
• Åtgärdade MP3-konvertering vid strömning: om strömmen redan är MP3 och användaren väljer en uttrycklig bithastighet, till exempel 128 kbps, kodar Sonarpad nu om till vald bithastighet i stället för att hoppa över konvertering.
• Åtgärdade dokument från medietranskribering så att stängning nu frågar om de ska sparas och det föreslagna filnamnet återanvänder mediefilens namn i stället för textens första rad.
• Åtgärdade `Alt+Shift+L`: det öppnar nu korrekt kapitellistan under uppspelning.
• Åtgärdade `Alt+Shift+T`: det startar nu ”Transkribera aktuellt ljud” i stället för att öppna Verktyg-menyn.
• Åtgärdade stopp i Spela-menyn: `.` beter sig nu som Stoppa och stoppar endast aktuellt spår i stället för att också lämna spelaren/avsnittet.
• Åtgärdade Spara-posten i Spela-menyn för media som öppnats från Senaste filer: när filen kommer från en lokal Sonarpad-cache visas den lokaliserade sparaåtgärden korrekt även där.
• När transkribering startas medan ljud redan spelas pausar Sonarpad nu ljudet automatiskt före transkriberingen.
• Åtgärdade ett fel där import av en Wikipedia-artikel kunde lyckas utan att artikeltexten visades på skärmen.
• Lade till stöd för inbäddade poddkapitel i lokala mediefiler, till exempel MP3-kapitelmetadata. När kapitel från flöde/URL saknas läser Sonarpad in kapitel från den nedladdade filen i bakgrunden, så uppspelningen startar direkt och kapiteldata tillämpas så snart de är klara.
• Åtgärdade kapitelladdning för nedladdade poddavsnitt som öppnas som vanliga lokala mediefiler: inbäddade kapitel finns nu även där, inte bara när uppspelning startas från Poddar-fönstret.
• Åtgärdade slutbehandling av MP3-ljudböcker för SAPI4 och SAPI5 så att slutfilen färdigställs korrekt och inte blir ofullständig eller skör efter långa exporter.
• Lade till en uttrycklig förloppsindikator för slutbehandling i alla lägen för ljudboksskapande. Efter skapandefasen meddelar och visar Sonarpad nu en separat slutbehandlingsfas med synligt förlopp.
• Åtgärdade inställningar för dialogröster: hastighet, tonhöjd och volym tillämpas nu korrekt för både första och andra dialogrösten under syntes.
• Förbättrade teckenkodningsidentifiering för japanska `.txt`-filer med en säker Shift_JIS/CP932-reserv för mojibake, samtidigt som befintligt beteende för UTF, diakritiska tecken och kinesiska bevaras.
• Intern säkerhetsrefaktorering: funktioner har gjorts säkra där det är möjligt och antalet rader med unsafe-kod har minskat betydligt.

Version 0.6.7 – 2026-03-02
Förbättringar
• Programmet kan nu hantera massiva Ersätt alla-åtgärder i stora filer med mycket många ersättningar.
• Uppdaterade polsk översättning tack vare DJ Graco.
• Lade till litauisk översättning.
• Lade till kinesisk översättning.
• Frekventa betaversioner kommer nu att publiceras i projektets Releases-avsnitt så att användare kan testa ändringar före nästa stabila version.
• Lade till `Ctrl+.` för att infoga ett ellipstecken (…).
• Förbättrade stöd för poddkapitel: kapitelnavigering fungerar mer tillförlitligt även för direkta/strömmande avsnitt där kapitel inte finns i MP3-filen, genom metadata från flöde/URL när sådana finns. Lade till `Ctrl+Alt+PageUp` för föregående kapitel och `Ctrl+Alt+PageDown` för nästa.
• Omorganiserade Sonarpads utdatamappar under `Documents\Sonarpad`: filer sparas nu i `audiobooks`, `documents`, `recordings` och `media`, med automatisk migrering från äldre sökvägar.
• Förbättrade stöd för mycket stora textfiler, inklusive 60 MB: smidigare öppning och radvis navigering, särskilt med skärmläsare.
• Uppdaterade guider för alla språk och lokaliseringsresurser i hela appen, inklusive donationstexter och NSIS-installationsöversättningar, med nya förenklad kinesiska och litauiska installationssträngar samt slutförd ukrainsk installation.
• Lade till global proxy för nätverk, HTTP/HTTPS och SOCKS5/SOCKS5H, för onlinefunktioner med validering när Alternativ sparas. Ogiltiga proxyn varnas och tas bort automatiskt.
• Lade till ”Spela strömmande ljud...” i Verktyg: klistra in en URL, YouTube eller direkt medialänk, välj utdataformat och kvalitets-/bithastighetsprofil, inklusive originalkvalitet för MP3 och MP4, och spela direkt i Sonarpads ljudspelare.
• Lade till stöd för systemets Play/Pause-mediatangent på headset/tangentbord: den styr både medieuppspelning och paus/fortsättning av textläsning, med prioritet för media när båda är aktiva.
• Lade till ”Rensa senaste filer” under Arkiv > Senaste filer.
• Utökade bithastighetsalternativen i Konvertera ljud och poddinspelning med 64/96 kbps och MP3 upp till 320 kbps, med motsvarande validering och kodarhantering.
• Utökade tidsalternativen för uppdelning av ljudböcker upp till 60 minuter.
• Förbättrade uppdelning av ljudböcker efter delar: antal delar kan nu anges manuellt, validerat från 1 till 100.
• Lade till Visa > Skrivskyddat läge för att skydda redigerartext från oavsiktliga ändringar samtidigt som dokumentet kan läsas och navigeras fullt ut.
• Lade till en tillgänglig förloppsindikator under programuppdateringar så att skärmläsare kan följa nedladdningen i realtid.
• Lade till en tyst statusrad i huvudfönstret som visar tecken, ord och rad/kolumn utan att störa NVDA-fokus.
• Lade till en snabb växel för Radbrytning i Visa-menyn.
• Lade till Redigera > Text-åtgärder för indrag och minska indrag, med `Ctrl+Shift+.` och `Ctrl+Shift+,`, eftersom Tab används för röstpanelen när ”Visa röster i redigeraren” är aktivt.
• Lade till lokaliserat datum/tid i RSS-artiklar och poddavsnitt, formaterat efter aktuellt gränssnittsspråk.
• Lade till en RSS-snabbmenyåtgärd för att dela vald artikel via e-post.
• Lade till detaljerade bekräftelsealternativ för borttagning av RSS/podd i Alternativ > RSS och poddar: RSS (flöde/artikel/båda/inget) och poddar (podd/avsnitt/båda/inget).
• Lade till konfigurerbar snabb RSS-kopiering med Ctrl+C: kopiera titel, URL, artikelinnehåll eller allt tillsammans.
• Enhetlig skapande av RSS-källor: ”Lägg till källa” accepterar nu både direkta flödes-URL:er och nyckelord, som automatiskt skapar Google News-RSS, i stället för en separat nyckelordssökning.
• Ctrl+A meddelar nu när markeringen är klar för tydligare skärmläsaråterkoppling.
• Lade till Shift+F3 för ”Sök föregående”, som komplement till F3 ”Sök nästa”.
• Förbättrade återkoppling vid ersättning med korrekt singular/plural, till exempel ”1 ersättning gjord” respektive ”2 ersättningar gjorda”.
• Lade till val av språk för ordboksuppslagning, med Automatisk som standard och möjlighet att välja manuellt.
• Lade till fliken Kortkommandon i Alternativ för att anpassa tangentkombinationer, med konfliktidentifiering som varnar när ett kortkommando redan används.
• Lade till grundläggande kommandoradsstöd: `-h`/`--help` visar hjälp och `--version` visar programversionen.
• Förtydligade manuell hastighet och tonhöjd: manuella fält använder nu en skala centrerad på 100, där 100 är normalvärdet.
• Förbättrade val av Microsoft-röster i Alternativ > Röst och röstpanelen i redigeraren med en lokaliserad språkkombinationsruta. Läget endast flerspråkiga röster behåller en enda ogrupperad lista och döljer språkfältet.
• Lade till konfiguration av dialogröster i Alternativ > Röst med full Tab-navigering och samma röstmodell som huvudgränssnittet: motor, Edge-språkfilter, röst och märkta hastighet/tonhöjd/volym. En valfri andra dialogröst med samma kontroller kan användas för alternerande dialog. Reglerna sparas i `.ini`, så dokumenttexten ändras inte.
• Förbättrade etiketten Ångra: Redigera > Ångra visar nu vilken åtgärd som ångras, till exempel textredigering, citera/avcitera rader eller rösttagg, och förblir inaktiverad när inget kan ångras.

Felkorrigeringar
• Åtgärdade öppning av RTF: `.rtf` tolkas nu och visas som läsbar vanlig text i stället för rå RTF-kod som `{\rtf1...}`.
• Åtgärdade kinesiska textfiler kodade i GB18030/GBK så att de identifieras och avkodas korrekt utan mojibake.
• Förbättrade skapande av M4B-ljudböcker med kapitelmetadata och kapitelmarkörer och åtgärdade chipmunk-problemet med hög ton/hastighet.
• Åtgärdade bithastighetsgränssnittet i dialogen för ljudbokssparande: hårdkodade italienska etiketter togs bort och 64 kbps lades till.
• Åtgärdade Spara alla (`Ctrl+Shift+S`): alla öppna ändrade dokument upptäcks nu tillförlitligt, även osparade/nya flikar, och sparas eller får Spara som vid behov.
• Åtgärdade ordningen i Google News RSS: artiklar visas nu i fallande publiceringsdatum, nyaste först, när datum finns.
• Åtgärdade NVDA-etikettkoppling i ordboksfönstret så att sökfält och språkkombination meddelar rätt etiketter.
• Åtgärdade tangentbordshantering i RSS/Podd-egenskaper: Tab/Shift+Tab når OK, Enter aktiverar OK, Esc stänger säkert och fokus återgår korrekt till RSS/Podd-listan.
• Åtgärdade ångrahistorik för RSS/Podd: Ctrl+Z stöder nu ångra i flera nivåer för borttagningar, artiklar/avsnitt och källor, inte bara senaste åtgärden.
• Förbättrade återkoppling vid borttagning i RSS/Podd med tydliga statusmeddelanden.
• Förbättrade fokus efter borttagning/ångra i RSS/Podd: RSS fokuserar nu säkert första flödet vid behov och undviker upprepade skärmläsarmeddelanden vid fördröjd ommarkering.

Version 0.6.6 – 2026-02-13
Förbättringar
• Lade till ”Autoformatera för TTS” i Redigera-menyn för att snabbt förbereda text för tal genom att ta bort markdown/citat och sammanfoga radbruten text.
• Förbättrade infogning av rösttaggar: när text är markerad tillämpas taggar nu korrekt på både enradiga och flerradiga markeringar.
• Lade till ett alternativ för standardmapp för sparade ljudböcker i ljudinställningarna (standard: Documents\\Sonarpad Audiobooks).
• I dialogrutan för att spara ljudböcker har ett nytt alternativ lagts till när delning är aktiverad. Det är aktiverat som standard och skapar en särskild undermapp för delarna, så att resultatet blir mer organiserat.
• Export av ljudböcker sparar nu MP3 i stereo med den bitrate som användaren valt för Edge-, SAPI5- och SAPI4-röster.
• Lade till stöd för 32-bitars SAPI5-röster via bridge, så att röster som bara finns i 32-bitarsmotorer också kan användas i Sonarpad.
• Röstfunktionerna har samlats i en särskild meny, ”Röst och ljud”, och ”Konvertera ljud” har lagts till/förtydligats för konvertering av alla mediaformat som stöds till MP3, AAC, OGG, Opus, FLAC, WAV och AIFF.
• Lade till möjlighet att ta bort enskilda RSS-artiklar och poddavsnitt (Delete + snabbmeny med bekräftelse) utan att ta bort hela RSS-/podd-källan, samt möjlighet att ångra den senaste borttagningen av en artikel, ett avsnitt eller en hel källa.
• Lade till export av RSS-flöden till OPML i RSS-fönstret, så att aktuella RSS-källor enkelt kan sparas och importeras igen.
• Lade till ”Sök RSS efter nyckelord” i RSS-fönstret. Ett nyckelord skapar nu automatiskt en Google News-RSS-adress och öppnar dialogrutan Lägg till källa med fälten ifyllda, så att ett nyckelordsflöde kan skapas i ett steg.
• Lade till serbisk översättning, tack till Mila Kuran.
• Lade till ukrainsk översättning, tack till Ivan Shtefuriak.
• Lade till öppning av flera mediefiler samtidigt: om flera mediefiler markeras/öppnas skapas nu en uppspelningskö i stället för att den aktuella filen ersätts.
• Lade till variabla hopp under uppspelning: med ett grundhopp på 1 minut flyttar Vänster/Höger 60 sekunder, Shift+Vänster/Höger 20 sekunder och Ctrl+Vänster/Höger 3 minuter.
• Lade till kortkommandon för föregående/nästa spår i spelaren: Ctrl+PageUp och Ctrl+PageDown.
• Lade till ”Återställ volym” och samlade återställningsåtgärderna i en särskild undermeny ”Återställ” under Uppspelning tillsammans med ”Återställ hastighet” och ”Återställ tonhöjd”.
• Förbättrade installationsprogrammet: setup.exe låter nu användaren välja mellan att associera alla filtyper som stöds eller välja filändelser manuellt; MSI visar nu val per filändelse i funktionsträdet (alla är fortfarande aktiverade som standard).
• Lade till en ny meny ”Fönster” med ”Öppna dokument...” för att snabbt växla till valfri öppen fil.
• Uppdaterade Visa > Teckensnitt: den gamla väljaren har ersatts av en snabb undermeny med vanliga teckensnitt (Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman, Georgia) och behåller aktuell textstorlek.
• Förbättrade meddelanden i RSS/Podd med två statusnivåer: källor med uppdateringar annonserar ”nya objekt”, medan enskilda RSS-artiklar och poddavsnitt annonserar ”oläst”/”ospelat”. Detta kan stängas av i Alternativ.
Buggfixar
• Åtgärdade textutvinning från EPUB-böcker med inbäddade HTML-kommentarer (<!-- ... -->): kapiteltexten läses nu korrekt i stället för att delvis eller helt hoppas över.
• Åtgärdade spanska Wiktionary-sökningar och ordbokens cache: spanska uppslag som ”agua” laddas nu korrekt och gamla cacheposter med ”Word not found” återanvänds inte längre.
• Åtgärdade teckenkodningen vid import av RSS-artiklar från vissa spanska källor (t.ex. El Mundo): accenttecken och ”ñ” bevaras nu korrekt i den tillfälliga redigeraren.
• Åtgärdade ANSI-avkodning av centraleuropeiska filer (t.ex. tjeckiska/polska): Sonarpad skiljer nu bättre mellan UTF-8 och ANSI och väljer rätt teckentabell, inklusive Windows-1250, för att undvika trasiga diakritiska tecken.
• Åtgärdade lagring av RSS-källor vars URL innehåller frågeparametrar (t.ex. `rss.aspx?c=...`): de sparas och återställs nu korrekt efter omstart av Sonarpad.
• Åtgärdade öppning av Google Drive-pekfiler (`.gdoc`, `.gsheet`, `.gslides`) från Utforskarens snabbmeny: om direkt läsning misslyckas med ”Incorrect function (os error 1)” används nu skalöppning så att dokumentet ändå öppnas korrekt.
• Åtgärdade läsning av äldre Excel 2010-filer `.xls`: gamla binära Excel-filer identifieras och avkodas nu korrekt i stället för att visa förvrängd text (t.ex. `ÐÏ_à¡±...`).
• Åtgärdade flödet för stavningsmeddelanden: felstavningar annonseras nu igen när text granskas senare, och samma fel rapporteras på nytt om det tas bort och skrivs igen.
• Åtgärdade radbaserade textåtgärder (t.ex. Ctrl+Q / Ctrl+Shift+Q, sortera/vänd/unika/sammanfoga rader): markering av en enda rad med Shift+Ned sammanfogar eller kapar inte längre intilliggande rader.
• Åtgärdade flerradsbeteendet för radbaserade textåtgärder (Ctrl+Q / Ctrl+Shift+Q och liknande): RichEdit-markeringar med endast CR som avgränsare normaliseras nu korrekt, så att alla markerade rader bearbetas utan att första tecken kapas.
• Utökade normalisering av TTS-indata för synliga blankstegssymboler (␠/U+2420, ␣/U+2423, ␉/U+2409, ␊/U+240A, ␍/U+240D, ␤/U+2424) för att förhindra upprepad styckeuppspelning med flerspråkiga röster.
• Förfinade saneringen av Edge TTS-text med en gemensam valideringskedja: konstiga/osynliga blanksteg normaliseras, långa skiljeteckensekvenser som ”...”, ”!!!”, ”???” komprimeras och segment som bara innehåller skiljetecken hoppas över för att förhindra uppspelningsloopar.
• Åtgärdade annonsering av uppspelningstid (Ctrl+I) för MP3-/poddströmmar: aktuell tid begränsas nu till spårets längd och uppspelningen stoppas automatiskt om positionen passerar slutet.
• Utökade installationsprogrammets lokalisering: setup.exe innehåller nu fler språk (tjeckiska, polska, franska, serbiska), medan MSI behålls som ett enda en-US-paket för att undvika förvirring vid releaser.
• Åtgärdade avinstallationsrensning för snabbmenyposter: ”Öppna med Sonarpad” tas nu bort tillförlitligt även i äldre registerscenarier.
• Åtgärdade tillförlitligheten för paus/fortsätt i SAPI5: F4 pausar nu korrekt och fortsättning återgår till förväntad position i stället för att börja om från början.
• Åtgärdade flödet paus + sökning + fortsätt för media: efter paus och förflyttning med Vänster/Höger fortsätter Mellanslag nu tillförlitligt från aktuell position i stället för att stoppa eller börja om.

Version 0.6.5 – 2026-02-07
Förbättringar
• Förbättrade den spanska översättningen, tack till Arturo Fernandez Rivas.
• Lade till ett alternativ för att dela EPUB-ljudböcker per kapitel.
• RSS-importer använder nu en särskild tillfällig flik med lokaliserad titel; Spara som gör den till ett vanligt dokument.
• Skärmläsarmeddelanden skickas nu även till JAWS när det finns tillgängligt.
Buggfixar
• Läsning från markören (F5) börjar nu exakt vid markören. Tidigare kunde den börja ett par rader ovanför eftersom markörens offset inte motsvarade CRLF-/UTF-16-positionerna.
• Åtgärdade ett omritningsproblem där text före markeringen tillfälligt kunde försvinna när man skrev över markerad text tills markeringen flyttades.
• Åtgärdade EPUB-kapitelanalys så att omslag eller sidor som bara innehåller bilder inte längre ger uppläst CSS (t.ex. ”padding”) eller titeln ”Sconosciuto”.
• Åtgärdade tidsbaserad delning av EPUB-ljudböcker med Edge TTS när tomma/för stora segment gav felet ”Edge audio not sent”.
• RSS-artiklar avkodar nu HTML-entiteter (t.ex. &quot;, &amp;, &lt;, &gt;).
• Spara/Spara som föreslår nu det befintliga filnamnet när ett format som inte kan skrivas över (t.ex. EPUB) sparas, i stället för textens första rad.
• Åtgärdade ett fel där poddar med nya avsnitt inte annonserades som ospelade och bytte etiketten ”Unheard” till ”Unplayed” för en mer professionell benämning.

Version 0.6.4 – 2026-02-05
Förbättringar
• Programmet har bytt namn till Sonarpad för att betona ljud som programmets huvudsakliga fokus.
• Lade till val av ljudspår i menyn Uppspelning för mediefiler med flera ljudspår, exempelvis MKV-filer med flera språk.
• Poddar visar nu tydligt ohörda avsnitt med prefixet ”Unheard” före namnet.
• Ny taggbaserad växling mellan röster i text. Exempel:
  - Microsoft-röster (Edge): <voice edge it-IT-IsabellaNeural>Hello</voice>
  - SAPI5-röster: <voice sapi5 Microsoft Helena Desktop>Hello</voice>
  - SAPI4-röster: <voice sapi4 #1>Hello</voice>
  - Med hastighet/tonhöjd/volym: <voice edge it-IT-ElsaNeural speed=-20 pitch=-5 volume=-10>Hello</voice>
• Utökade poddkategorier.
• Förbättrade PDF-läsning med automatisk reservlösning via PDFium.
• Förbättrade artikelanalysatorn i fall där innehållet inte lästes fullständigt.
• Lade till återställning av tonhöjd i menyn Uppspelning.
• Lade till en snabbmenyåtgärd för att skapa en ljudbok av markerad text.
• Lade till delning av ljudböcker efter varaktighet med möjlighet att välja namnet på den första filen.
• Lokaliserade författaretiketten vid artikelläsning (t.ex. ”by”, ”di”, ”par”).
• Lade till indragsalternativ (tabbar/mellanslag med bredd) och Tab/Shift+Tab för in-/utdrag av markerade rader.
• Åtgärdade Markdown-rensning så att listpunkter med `*` hanteras när bevarande av punktlistor är avstängt.
• Lade till ett alternativ för att använda det äldre namnet ”Novapad” i fönstertiteln och genvägarna i Start-menyn.
Buggfixar
• Åtgärdade ett fel där SAPI4-ljudböcker kunde skapas annorlunda än förväntat.
• Åtgärdade ett fel där sökning förbi slutet av en mediefil startade uppspelningen från början igen.
• Sök i filer: Enter på ett resultat öppnar nu vid rätt utdragsposition och Esc går tillbaka till resultaten.
• Alternativ: förbättrade den visuella layouten på flikarna Allmänt, Röst, Redigerare och Ljud för att förhindra saknade eller avklippta kontroller.
• Åtgärdade ett bokmärkesproblem när uppspelningshastigheten ändrades.
• Åtgärdade att Podcast Index-kategorier inte visades korrekt.
• Åtgärdade att apostrofer störde läsningen genom att ta bort separat dialogläsning; rösttaggar används i stället.

Version 0.6.3 – 2026-01-30
Förbättringar
• Förbättrade mikrofonidentifieringen.
• Lade till omedelbar uppspelning för alla format.
Buggfixar
• Åtgärdade en krasch i fönstret för poddkategorier.

Version 0.6.2 – 2026-01-30
Nya funktioner
• Lade till stöd för att köra filer (Shift+F5). Användaren kan välja en tolk, exempelvis Python, i Alternativ, söka efter den på datorn och köra det aktuella skriptet med Shift+F5. HTML-filer öppnas i webbläsaren.
• Lade till stöd för Google Docs-pekfiler (.gdoc, .gsheet, .gslides), som automatiskt öppnas i standardwebbläsaren.
• Lade till stöd för ljudboksformatet M4B (Apple/AAC).
• Lade till ”Visa avsnitt” i snabbmenyn för poddsökresultat för att bläddra bland och spela avsnitt utan att prenumerera.
• Lade till ”Gå till rad” (menyn Redigera eller Ctrl+J) för att snabbt hoppa till ett visst radnummer.
• Lade till snabbmenyalternativ för att ordna RSS-flöden och poddar alfabetiskt eller efter datum.
• Lade till vietnamesiska standard-RSS-flöden.
• Lade till en mikrofontestruta i inspelningsdialogen för att kontrollera nivåer före start.
• Lade till ”Visa beskrivning” för poddavsnitt i snabbmenyn.
• Lade till stöd för fler ljud-/videoformat via FFmpeg: mkv, avi, mov, m4v, webm, mpg, ts, wmv, flv, vob, 3gp, flac, ogg, wma, aiff.
• Lade till stöd för synkroniserad uppläsning av undertexter (srt, vtt, ass, sub, sbv, lrc, smi) med NVDA eller vald röst. Programmet söker efter en undertextfil med samma namn som mediefilen. ”Importera undertexter” och ”Ta bort undertexter” har lagts till i menyn Uppspelning för filer med andra namn.
• Lade till filassociationer för alla nya ljud-/videoformat som stöds i snabbmenyn ”Öppna med Sonarpad”.
• Lade till inställning för tonhöjd för alla filer.
• Lade till alternativ i Allmänna inställningar för att aktivera eller inaktivera anonyma felrapporter. Lade till en Hjälp-menyåtgärd för att skapa en diagnostik-ZIP.
• Lade till möjlighet att använda en annan röst för dialoger, både vid direktläsning och skapande av ljudböcker.
• Lade till bläddring bland poddkategorier för att utforska poddar efter kategori (företag, konst, sport osv.).
Förbättringar
• Om en ljud-/videofil öppnas från Utforskaren visas nu spelaren direkt i stället för textredigeraren.
• Tog bort OCR-frågan för otillgängliga PDF-filer; OCR körs nu automatiskt för bättre hastighet och användarupplevelse.
• Förbättrade Tillgänglig terminal: NVDA-läsningen minns nu den senast upplästa raden för bättre kontinuitet.
• SAPI4: skapande av ljudböcker är nu helt parallelliserat och nästan omedelbart. En fråga för att välja antal samtidiga processer har lagts till.
• SAPI4: tog bort flaskhalsen WAV-till-MP3 genom att konvertera segment parallellt under syntesen.
• SAPI4: förbättrade felhantering och automatisk rensning av tillfälliga filer.
• Sök-dialogen: bytte namn på ”Regex” till ”Reguljärt uttryck” för tydlighet och lade till saknade översättningar för sökalternativen.
• M4B-ljudböcker: bättre hantering av utdata; delning efter delar/markörer skapar nu en enda M4B-fil med korrekta metadatakapitel, inklusive titel och författare.
• Spelare: åtgärdade precision för bokmärken och tidsannonsering när uppspelningshastigheten inte är 1,0x.
• Återställde Ctrl+Tab och Ctrl+Shift+Tab-navigering i Alternativ.
• Lade till ett alternativ i Uppspelning för att omedelbart återställa hastigheten till Normal (1,0x).
• Uppdaterade alla beroenden till de senaste versionerna för bättre prestanda och stabilitet.
• Integrerade FFmpeg med dynamisk DLL-laddning för kompatibilitet utan att blockera starten.
• Uppdaterade filter för poddnedladdningar så att de omfattar de nya ljud-/videoformaten.
• Förhindrade Ctrl+S från att spara ljud-/videofiler för att undvika filskador.
• Gjorde import av YouTube-transkript mer robust och motståndskraftig.
• Förbättrade delning av ljudboksdelar så att ingen text går förlorad.
• Installationsprogrammet är nu helt flerspråkigt och stöder italienska, engelska, spanska, portugisiska, svenska och vietnamesiska utifrån systemets språk. Engelska används som standard på system som inte stöds.
• Poddkategorier: Enter på en kategori bekräftar nu valet, motsvarande OK-knappen.
• Förbättrade systemet för hängningsdetektering för att undvika falsklarm när modala dialogrutor, exempelvis felmeddelanden eller ”text hittades inte”, är öppna.
Korrigeringar
• Åtgärdade ett fel där ändringsloggen inte öppnades vid start.
• Åtgärdade ett fel där OCR-frågan inte visades för otillgängliga PDF-filer som öppnades från Utforskaren.
• Åtgärdade ett startfel som kunde orsaka fokusförlust eller att fönstret stängdes direkt efter öppning.
• Åtgärdade ett kritiskt fel i regex-sökning som förhindrade textsökning, inklusive problem med ”Börja om från början” och alternativet ”Punkt matchar radbrytning” med Windows-radslut.
Lokalisering
• Lade till polsk översättning.
• Lade till fransk översättning.
• Lade till tjeckisk översättning, tack till Radek Žalud och Jiri Holzinger.

Version 0.6.1 – 2026-01-20
Korrigeringar
• Åtgärdade ett fel där aktivering av ”Visa röster i redigeraren” gjorde att podduppspelning stoppades.
• Åtgärdade ett problem där vissa poddar inte kunde läggas till via URL eftersom adressen kapades.
• Åtgärdade ett fel där vanliga URL-adresser inte längre kunde läggas till i RSS-funktionen.
• Åtgärdade ett problem där Wikipedias språkalternativ visades flera gånger på olika inställningsflikar.
• Tog bort skapande av debugfiler som felaktigt genererades även i release-läge.
Förbättringar
• Förbättrade stödet för Microsoft-röster, som nu använder en särskild uppspelningsmetod med en annan user agent.
• Lade till stöd för MP4-filer.

Version 0.6.0 – 2026-01-20
Nya funktioner
• Lade till stavningskontroll. Från snabbmenyn kan användaren kontrollera om aktuellt ord är korrekt och annars få stavningsförslag.
• Lade till import och export av poddar via OPML-filer.
• Lade till stöd för Podcast Index-sökning utöver iTunes. Användaren kan ange sin kostnadsfria API-nyckel och hemlighet, som skapas med endast en e-postadress.
• Lade till stöd för SAPI4-röster, både för direktläsning och skapande av ljudböcker.
• Lade till automatisk OCR-reserv för otillgängliga PDF-filer: om ingen extraherbar text hittas känns dokumentet igen med OCR.
• Lade till ordboksstöd via Wiktionary. Med Program-tangenten visas definitioner och, när de finns, synonymer och översättningar till andra språk.
• Lade till import av Wikipedia-artiklar med sökning, resultatval och direkt import till redigeraren.
• Lade till Shift+Enter i RSS-modulen för att öppna en artikel direkt på originalwebbplatsen.
Förbättringar
• Mikrofonvalet respekteras nu alltid av programmet.
• I poddfönstret annonserar NVDA omedelbart ”laddar” när Enter trycks på ett avsnitt, som bekräftelse på åtgärden.
• I poddsökresultat prenumererar Enter nu på vald podd.
• Åtgärdade och förbättrade etiketterna för Ctrl+Shift+O och Podd Ctrl+Shift+P.
• Uppspelningshastighet och volym sparas nu i inställningarna och behålls mellan alla ljudfiler.
• Lade till en särskild cachemapp för poddavsnitt. Avsnitt kan behållas via ”Behåll podd” i menyn Uppspelning. Cachen rensas automatiskt när den överskrider användarens angivna storlek (Alternativ → Ljud).
• Förbättrade hämtning av RSS-artiklar avsevärt med libcurl-impersonering av Chrome- och iPhone-profiler, vilket ger kompatibilitet med cirka 99 % av webbplatserna.
• Lade till status läst/oläst för RSS-artiklar med tydlig markering i RSS-listan.
• Ersätt alla rapporterar nu antalet genomförda ersättningar.
• Lade till knappen Ta bort podd när poddbiblioteket navigeras med Tab.
Korrigeringar
• Tog bort den överflödiga posten ”väntande uppdatering” från Hjälp-menyn eftersom uppdateringar redan hanteras automatiskt.
• Åtgärdade ett fel där Ctrl+S på en öppnad MP3-fil sparade och skadade filen.
• Åtgärdade ett UI-problem där ”Batch-ljudböcker” visades som ”(B)… Ctrl+Shift+B” genom att ta bort den överflödiga etiketten.
• Åtgärdade smarta citattecken: när de är aktiverade ersätts vanliga citattecken nu korrekt.
• Åtgärdade ett fel där ”Gå till bokmärke” återställde uppspelningshastigheten till 1,0.
• Åtgärdade ett problem där redan nedladdade poddavsnitt laddades ned igen i stället för att använda den cachade versionen.
Kortkommandon
• F1 öppnar nu hjälpguiden.
• F2 söker nu efter uppdateringar.
• F7 / F8 hoppar nu till föregående eller nästa stavfel.
• F9 / F10 växlar nu snabbt mellan favoritröster.
Utvecklarförbättringar
• Fel ignoreras inte längre tyst: alla `let _ =`-mönster har tagits bort och fel hanteras nu uttryckligen genom vidarebefordran, loggning eller lämpliga reservlösningar.
• Projektet misslyckas nu med kompileringen om det finns varningar: både cargo check och cargo clippy måste passera utan anmärkningar, med striktare lint-regler och `allow` borttaget där det är möjligt.
• Egna implementationer som strlen-/wcslen-liknande hjälpfunktioner har tagits bort. Längder för strängar och UTF-16-buffertar härleds nu från Rust-ägda data i stället för att minne skannas.
• DLL-hanteringen har städats och samlats kring libloading, utan egen laddningslogik eller PE-analys.
• Egna byte-parserhjälpare har tagits bort; all byteparsning använder nu standardfunktionerna from_le_bytes / from_be_bytes på kontrollerade slices.
Dessa ändringar minskar onödig unsafe-användning, eliminerar potentiellt odefinierat beteende och gör kodbasen mer idiomatisk, robust och underhållbar.

Version 0.5.9 - 2026-01-13
Nya funktioner
• Lade till omordning av RSS via snabbmenyn (upp/ned/till position) med kontroll av ogiltig position.
• Lade till en snabbmeny för artiklar med öppning av originalwebbplats och delning via WhatsApp, Facebook och X.
• Lade till Esc för att återgå från importerade artiklar till RSS-listan.
• Lade till poddläge: sök, prenumerera och lyssna; ordna prenumerationer; Esc stoppar uppspelning och återgår till listan; Enter på ett avsnitt startar uppspelning.
• Lade till styrning av uppspelningshastighet för poddar och MP3-filer.
• Lade till Ctrl+T för att hoppa till en viss tid.
• Lade till en knapp för röstförhandslyssning efter volymkombon.
• Lade till sök och ersätt med reguljära uttryck i Notepad++-stil.
• Lade till RSS-import från OPML- och TXT-filer.
• Lade till ett alternativ för att aktivera ”Öppna med Sonarpad” i Utforskaren, även för portabla versioner.
Förbättringar
• Förbättrade val av rösthastighet/tonhöjd/volym med respekt för TTS-motorns maxgränser.
• Flera RSS-förbättringar så att alla artiklar kan laddas ned utan att NVDA-fokus flyttas under uppdatering.
• Förbättrade ljuduppspelning med en särskild meny, Ctrl+I för tidsannonsering och volym upp till 300 %.
• Lade till saknade kortkommandon för vissa funktioner.
• Omorganiserade Redigera-menyn med en undermeny för textrensning.
• Omorganiserade Alternativ i flikar med navigering via Ctrl+Tab och Ctrl+Shift+Tab.
• RSS-läsaren hämtar nu hela artikelinnehållet så att det motsvarar webbläsarvyn.
Korrigeringar
• Åtgärdade att Markdown-rensning tog bort siffror i början av rader.
• Åtgärdade att AltGr+Z utlöste ångra.
• Åtgärdade avbrytning av ljudboksinspelning så att den stoppar snabbt.
Lokalisering
• Lade till vietnamesisk översättning, tack till Anh Đức Nguyễn.

Version 0.5.8 - 2026-01-10
Nya funktioner
• Lade till volymkontroll för mikrofon och systemljud vid inspelning av poddar.
• Lade till en ny funktion för att importera artiklar från webbplatser eller RSS-flöden, inklusive de viktigaste flödena för varje språk.
• Lade till en funktion för att ta bort alla bokmärken för aktuell fil.
• Lade till en funktion för att ta bort dubblerade rader och dubblerade intilliggande rader.
• Lade till en funktion för att stänga alla flikar eller fönster utom det aktuella.
• Lade till posten Donationer i Hjälp-menyn på alla språk.
Förbättringar
• Förbättrade den tillgängliga terminalen för att förhindra vissa krascher.
• Förbättrade och korrigerade snabbtangenter och kortkommandon i hela programmet.
• Åtgärdade ett problem där uppspelningen inte stoppades när ljudspelarens fönster stängdes.
• Lade till bekräftelsedialoger för viktiga åtgärder, exempelvis ta bort dubblerade rader, ta bort bindestreck vid radslut och ta bort alla bokmärken i aktuell fil. Ingen dialog visas om åtgärden inte är tillämplig.
• Lade till möjlighet att ta bort RSS-flöden/webbplatser från biblioteket genom att markera dem och trycka Delete.
• Lade till en snabbmeny i RSS-fönstret för att redigera eller ta bort RSS-flöden/webbplatser.
• Tog bort inställningen för att flytta konfigurationen till aktuell mapp. Programmet hanterar detta nu automatiskt beroende på plats: om exe-mappen heter ”sonarpad portable” eller exe-filen finns på en flyttbar enhet sparas inställningarna i exe-mappens `config`; annars i `%APPDATA%\\Sonarpad`, med exe-mappens `config` som reserv om den föredragna mappen inte är skrivbar.

Version 0.5.7 - 2026-01-05
Nya funktioner
• Lade till Batch-ljudböcker för att konvertera flera filer/mappar samtidigt.
• Lade till stöd för Markdown-filer (.md).
• Lade till val av teckenkodning när textfiler öppnas.
• Lade till ett alternativ i den tillgängliga terminalen för att annonsera nya rader med NVDA.
Förbättringar
• Ljudboksinspelning sparar nu direkt till MP3 när det formatet väljs.
• Användaren kan nu välja positionen för asterisken (*) för osparade ändringar i fönstertiteln.
• Förbättrade uppdateringssystemets robusthet i olika scenarier.
• Lade till ”Ta bort bindestreck” i Redigera-menyn för att rätta OCR-radbrytningar.

Version 0.5.6 - 2026-01-04
Korrigeringar
  Förbättrade Sök i filer så att Enter öppnar filen exakt vid det markerade utdraget.
Förbättringar
  Lade till stöd för PPT/PPTX (öppna som text).
  När format som inte är text öppnas sparas de nu som .txt för att undvika skadad formatering (PDF/DOC/DOCX/EPUB/HTML/PPT/PPTX).
  Lade till poddinspelning från mikrofon och systemljud (Arkiv-menyn, Ctrl+Shift+R).

Version 0.5.5 – 2026-01-03
Nya funktioner
• Lade till en tillgänglig terminal optimerad för stora mängder utdata och skärmläsare (Ctrl+Shift+P).
• Lade till en inställning för att spara användarinställningar i aktuell mapp (portabelt läge).
Korrigeringar
• Förbättrade utdrag i Sök i filer så att förhandsvisningen förblir rätt justerad mot träffen.

Version 0.5.4 – 2026-01-03
Förbättringar
• Åtgärdade Normalisera blanksteg (Ctrl+Shift+Enter).
• Lade till stöd för HTML/HTM (öppna som text).

Version 0.5.3 – 2026-01-02
Nya funktioner
• Lade till Sök i filer.
• Lade till nya textverktyg: Normalisera blanksteg, Hård radbrytning och Ta bort Markdown.
• Lade till Textstatistik (Alt+Y).
• Lade till nya listkommandon i Redigera-menyn:
• Ordna objekt (Alt+Shift+O)
• Behåll unika objekt (Alt+Shift+K)
• Vänd objekt (Alt+Shift+Z)
• Lade till Citera / Ta bort citat från rader (Ctrl+Q / Ctrl+Shift+Q).
Lokalisering
• Lade till spansk lokalisering.
• Lade till portugisisk lokalisering.
Förbättringar
• När en EPUB-fil är öppen växlar Spara nu automatiskt till Spara som och exporterar innehållet som .txt för att undvika att EPUB-filen skadas.

## 0.5.2 - 2026-01-01
- Lade till en ändringslogg.
- Lade till alternativ för Öppna med Sonarpad och filassociationer för format som stöds under installation.
- Förbättrade lokaliseringen av meddelanden, fel, dialogrutor och ljudboksexport.
- Lade till val av delar vid ”Dela ljudbok baserat på text”, med alternativet ”Kräv markören i början av raden”.
- Lade till import av YouTube-transkript med språkval, tidsstämpelalternativ och förbättrad fokushantering.

## 0.5.1 - 2025-12-31
- Automatiska uppdateringar med bekräftelse, förbättrad felhantering och meddelanden.
- Förbättringar av ljudboksexport (textbaserad delning, SAPI5/Media Foundation, avancerade kontroller).
- TTS-förbättringar (paus/fortsätt, ersättningsordbok, favoriter).
- Visa-menyn och paneler för röster/favoriter, textfärg och storlek.
- Standardspråk från systemets språk och förbättrad lokalisering.
- CI och Windows-paketering (artefakter, MSI/NSIS, cache).

## 0.5.0 - 2025-12-27
- Modulär refaktorering (redigerare, filhanterare, meny, sökning).
- Arbetsflöde för Windows-build/paketering och uppdateringar av README/licens.
- Åtgärdade TAB-navigering i Hjälp-fönstret.

## 0.5 - 2025-12-27
- Preliminär versionshöjning.

## 0.1.0 - 2025-12-25
- Första versionen: projektstruktur och README.
