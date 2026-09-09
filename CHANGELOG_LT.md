# Pakeitimų žurnalas

Versija 0.9.6 – 2026-09-09

1. Ištaisytas kai kurių SAPI4 balsų strigimas įjungus žymeklio sekimą. Jungiamasis procesas dabar laukia tikrosios sintezės pabaigos, o žymeklio sekimas lieka įjungtas.

2. Ištaisyta mikrofono ir sistemos garso sinchronizacija bei spragsėjimas dėl laiko žymų apvalinimo įrašant tinklalaides. Pataisos taikomos ir atskirai išsaugant garso šaltinius.

3. SAPI5 garsinių aprašymų sintezė dabar vyksta atskiruose procesuose. Nepavykus sintezė kartojama su mažesniu lygiagretumu, išsaugomi sėkmingi aprašymai ir įsimenama tinkama balso riba.

Versija 0.9.5 – 2026-09-07

DI garso aprašymas
1. Pagerintas atsparumas, kai Gemini laikinai grąžina `MALFORMED_RESPONSE` be tinkamo naudoti turinio. Sonarpad dabar bando lygiai tą pačią dalį dar kartą iki trijų kartų, trumpai palaukdamas tarp bandymų, užuot iš karto nutraukęs visą garso aprašymą. Įprastos Gemini reakcijos, saugos blokai, tokenų limito klaidos ir esamas kontrolinių taškų veikimas lieka nepakeisti.

2. Ištaisytas dar vienas retas `invalid Gemini chunk timeline` atvejis, pasitaikantis su kai kuriais vaizdo įrašais, kai FFmpeg paruošti segmentai pateikia nedidelį kaupiamą trukmės nuokrypį. Kai laiko juosta teisinga, Sonarpad palieka įprastą veikimą nepakeistą ir konservatyvų suderinimą naudoja tik tada, kai įprasta laiko juosta nepavyktų ir bendras nuokrypis yra mažas; dideli ar įtartini neatitikimai ir toliau baigiasi klaida.

3. Pridėta pasirenkama Sonarpad AI paslauga DI garso aprašymams. Naudotojai gali ir toliau naudoti savo Gemini API raktą arba pasirinkti Sonarpad paslaugą ir įvesti asmeninį Sonarpad kodą. Kodas apsaugomas naudojant Windows DPAPI. Paslaugos režimu paruošti vaizdo segmentai į Google įkeliami tiesiogiai iš naudotojo kompiuterio per laikiną Google įkėlimo URL; sonarpad.com negauna ir nesaugo vaizdo baitų. Backend tik suteikia prieigą, nustato Gemini modelį/Standard lygį ir naudojimo ribas, apskaito naudojimą bei grąžina Gemini atsakymą.

4. Patobulintas išsaugotų garsinio vaizdavimo projektų redagavimas. Dabar galima iš eilės pakeisti kelis aprašymus, nereikia kiekvieno pakeitimo taikyti iš karto. Pakeitimai lieka atmintyje; paspaudus „Taikyti tekstą“, Sonarpad patikrina kiekvieną pakeistą aprašymą pagal jo išsaugotą laiko intervalą ir tada kartu išsaugo visus tinkamus pakeitimus. Jei aprašymas netelpa į savo intervalą, židinys grąžinamas į jį neprarandant kitų laukiančių pakeitimų.

5. „Reguliuoti balsą“ pridėta iškart prieš „Kurti garsinį vaizdavimą“. Naujame lange galima pasirinkti sintezės variklį, balsą, greitį ir garsumą bei naudoti „Išbandyti balsą“. Paspaudus OK židinys grįžta tiksliai į „Reguliuoti balsą“, o pasirinktos reikšmės taikomos kuriamam garsiniam vaizdavimui. Jei langas nenaudojamas, sintezė lieka visiškai tokia pati kaip anksčiau.


6. Išplėsti išsaugotų garsinio vaizdavimo projektų balso valdikliai. Projekto redagavimo lange, be variklio ir balso, dabar galima keisti greitį, garsumą ir naudoti „Išbandyti balsą“. Pritaikius balso parametrus, Sonarpad iš naujo patikrina visus esamus aprašymus su naujais sintezės parametrais ir MP3 perkuria tik jei visi aprašymai vis dar telpa į išsaugotus laiko intervalus. Intervalai be dialogo, Visual Evidence laikai, Gemini aprašymai ir projekto laiko analizė panaudojami iš naujo, nevykdant AI analizės dar kartą.

Versija 0.9.4 – 2026-09-04

DI garso aprašymas
1. Ištaisyta problema su Matroska/WebM vaizdo įrašais, kuriuose yra neįprastai didelė vidinė pradžios laiko žyma ir dėl kurios garso aprašymo kūrimas galėjo sustoti su klaida „invalid Gemini chunk timeline“. Sonarpad dabar normalizuoja šaltinio failo ir Gemini dalių trukmę tik aptikęs šį neįprastą laiko žymų modelį, todėl įprasti vaizdo įrašai ir jau veikiantis garso aprašymo elgesys lieka nepakeisti.
2. Patobulintas garso aprašymo ducking, kad galutinis miksas skambėtų natūraliau. Originalus garsas dabar švelniai pritildomas dar prieš prasidedant balsui ir po aprašymo palaipsniui grąžinamas į įprastą lygį; labai arti viena kitos esančios aprašo frazės išlaiko stabilų foninį lygį, todėl garsas nebesvyruoja pirmyn ir atgal.
3. Ištaisyta problema, dėl kurios galutinis garso aprašymo eksportas galėjo nepavykti su kai kuriais AVI failais ir kitais šaltiniais, turinčiais 5.1 / daugiakanalį garsą, kai dekodavimas baigdavosi nepilnu persipynusiu garso kadru. Sonarpad dabar saugiai papildo tik tą paskutinį dalinį kadrą tiksliai tiek tylos mėginių, kiek būtina; jau pilni kadrai ir įprasti mono, stereo ar daugiakanaliai failai lieka nepakeisti.


Versija 0.9.3 – 2026-09-03

SAPI5 balsai
1. Ištaisyta problema, dėl kurios kai kurie vietiniai SAPI5 balsai galėjo nekalbėti, kai įjungtas žymeklio judėjimas, naudojant kelių balsų skaitymą arba kuriant garsines knygas / MP3. Sonarpad dabar naudoja patikimai Windows veikiančią SAPI5 sintezės į failą eigą, kurią vis dar galima atšaukti sintezės metu, o įprastas tiesioginis atkūrimas lieka nepakeistas.
2. Ištaisyta žymeklio padėtis skaitant dialogus keliais balsais. Sonarpad automatiškai dialogams įterpiamos balso žymos dabar laikomos tik atkūrimo metaduomenimis, o ne redaktoriaus simboliais, todėl po F4 arba F6 žymeklis nebeperšoka į priekį už tikrojo teksto. Skaitymas vienu balsu ir dokumente aiškiai įrašytos <voice> žymos lieka nepakeistos.

DI garso aprašymas
1. Iškart po „Gemini“ API rakto lauku pridėtas žymimasis langelis „Rodyti API raktą“. Pagal numatytuosius nustatymus jis išjungtas; įjungus visas raktas laikinai parodomas, kad būtų galima patikrinti, ar jis įklijuotas visas. Iš naujo atidarius langą raktas vėl paslepiamas.

Tinklalaidės ir Vikipedija
1. Išsaugojus tinklalaidės įrašą, Sonarpad dabar paklausia, ar atidaryti aplanką, kuriame yra išsaugotas failas, kaip jau daroma išsaugojus YouTube / srautinio perdavimo mediją.
2. Importuojant kitą Vikipedijos straipsnį į redaktorių, kuriame jau yra teksto, naujas straipsnis dabar pridedamas pabaigoje, o ne įterpiamas viršuje. Žymeklis perkeliamas į naujai importuoto straipsnio pradžią.


Versija 0.9.2 – 2026-09-02

AI garsinis vaizdavimas
1. Ištaisyta problema, dėl kurios AI garsinis vaizdavimas galėjo nepavykti galutinio MP3 eksportavimo metu, kai vaizdo įraše yra daugiakanalis garsas, pavyzdžiui, 5.1. Dabar Sonarpad automatiškai konvertuoja daugiakanalį garsą į stereo tik tada, kai to reikia MP3 kodavimui, nekeisdamas mono ar stereo eksporto.
2. Paleidžiant DI garsinį aprašymą vaizdo įrašui su keliais garso takeliais, Sonarpad dabar prieš apdorojimą paklausia, kurį takelį naudoti. Pasiekiamą išskleidžiamąjį sąrašą galima keisti rodyklių klavišais; OK pradeda garsinį aprašymą su pasirinktu takeliu, o Atšaukti uždaro garsinio aprašymo langą ir grąžina fokusą į Sonarpad redaktorių.

YouTube ir srautinis turinys
1. Ištaisyta problema, kai paleidus AI garsinį vaizdavimą vaizdo įrašui, esančiam 2-ame ar vėlesniame YouTube grojaraščio arba kanalo puslapyje, galėjo vėl atsidaryti YouTube pasirinkimo langas ir perimti fokusą iš garsinio vaizdavimo lango. Dabar Sonarpad teisingai uždaro pasirinkimo langą negrįždamas į ankstesnius puslapius.
Versija 0.9.1 – 2026-09-01

YouTube atsisiuntimai
• Ištaisyta problema, dėl kurios YouTube / srautinio turinio atsisiuntimo eigos langai galėjo pakartotinai grįžti į pirmą planą perėjus į kitą programą su Alt+Tab. Dabar atsisiuntimai tęsiami fone ir neperima fokuso.
• Pagerintas atsisiuntimo eigos prieinamumas. Grįžus į eigos langą ekrano skaitytuvai gali perskaityti dabartinę būseną ir procentus. Grojaraščiuose Sonarpad taip pat praneša dabartinio elemento numerį, bendrą elementų skaičių ir pavadinimą.
• Ištaisyti klaidingi watchdog programos strigimo pranešimai ilgų atsisiuntimų ir konvertavimų metu, kai eigos langas vis dar reagavo.
• Grojaraščių atsisiuntimui pridėtas formato išskleidžiamasis laukelis. Vaizdo įrašų sąraše paspaudus Tab galima pasirinkti MP4, MP3, M4A, OPUS, OGG, WAV arba FLAC prieš pradedant kelių failų atsisiuntimą.
• Pertvarkytas srautinio turinio išsaugojimas. Formatas ir kokybė dabar pasirenkami išsaugojimo metu, o ne pradiniame srautinio turinio paieškos lange. „Išsaugoti mediją“ atveria vieną Formato ir Kokybės dialogą, o grojaraščių atsisiuntime pateikiami abu išskleidžiamieji laukai.

DI garsinis vaizdavimas
• Ištaisyta problema, dėl kurios su kai kuriais MKV vaizdo įrašais galėjo nepasileisti DI garsinis vaizdavimas. Sonarpad dabar patikimiau apdoroja vaizdo įrašus su netaisyklingomis arba trūkstamomis laiko žymomis.

Versija 0.9.0 – 2026-08-31

DI garsinis vaizdavimas — svarbi nauja funkcija
• Įrankiai > Multimedija pridėta „Kurti garsinį vaizdavimą su DI“. Sonarpad analizuoja garsą, kad rastų vietas be dialogų, generuoja aprašymus su Gemini ir naudoja Sonarpad jau esančius kalbos sintezės variklius, vengdamas kalbėti virš dialogų.
• Pagerinta sinchronizacija tarp to, kas vyksta vaizdo įraše, ir sugeneruotų aprašymų, automatiškai tikrinant Gemini laiko žymas.
• „Įjungti išplėstas pauzes“ pagal numatytuosius nustatymus išjungta. Šią parinktį galima įjungti turiniui su daug dialogų arba mažai laisvos vietos, kad būtų galima įterpti ilgesnius aprašymus.
• Sonarpad gali bandyti atpažinti veikėjus ir vartoti jų vardus. Veikėjų katalogus galima išsaugoti tarp serialo epizodų, kad būtų geresnis tęstinumas.
• Projektus galima išsaugoti, vėliau redaguoti ir dar kartą eksportuoti nepergeneruojant visko su Gemini.
• Jei procesas nutrūksta, Sonarpad išsaugo pažangą ir gali tęsti garsinį vaizdavimą. Jei išnaudojama Gemini kvota, galima palaukti, pakeisti modelį arba sustabdyti neprarandant jau atlikto darbo.
• Lange galima pasirinkti kalbą, detalumo lygį, Gemini modelį, kalbos sintezės variklį ir balsą; pasirinktos nuostatos įsimenamos.
• Modulis pasiekiamas visomis 17 Sonarpad kalbų. Generavimo metu sąsajoje rodomas tik progresas, dabartinė būsena ir Atšaukti; baigus MP3 galima tiesiogiai atidaryti vidiniame leistuve.

El. knygos ir dokumentai
• Pridėtas DRM neapsaugotų Kindle failų MOBI, AZW ir AZW3 importas; tekstas ir skyriai pasiekiami redaktoriuje ir dokumento rodyklėje.
• Pridėtas DAISY 2.02 ir DAISY 3 palaikymas. DAISY garsinės knygos naudoja vidinį Sonarpad leistuvą ir paiso skyrių navigacijos bei atkūrimo ribų.
• Kindle ir DAISY failai importuojami neperrašant originalo; DRM apsaugotos Kindle knygos aiškiai atmetamos.
• Ištaisyta EPUB „Išsaugoti kaip“: pasirinkus TXT ar kitą formatą dabar naudojamas pasirinktas plėtinys, o originalus EPUB lieka susietas su atidarytu dokumentu.

RSS ir straipsniai
• Pridėtas kelių RSS straipsnių pasirinkimas, kad vienu veiksmu būtų galima pašalinti kelis straipsnius.
• RSS dabar palaiko tikrus aplankus, kurie išsaugomi importuojant ir eksportuojant OPML, įskaitant tuščius aplankus.
• Sklaidos kanalus dabartiniame aplanke galima pertvarkyti komandomis Perkelti aukštyn, Perkelti žemyn, Perkelti į viršų, Perkelti į apačią ir Perkelti į poziciją.

Prieinamumas, vadovai ir sąsaja
• Sonarpad vadovai pertvarkyti ir papildyti rodykle, taip pat pridėtas išsamus DI garsinio vaizdavimo vadovas.
• Ištaisyta vokiečių vertimo problema, dėl kurios galėjo nepasirodyti Atidaryti, Išsaugoti kaip ir kiti failo pasirinkimo dialogai.

Balsai ir kalbos
• Atsisiunčiamas Google TTS katalogas išaugo nuo 104 iki 156 paketų ir nuo 53 iki 81 kalbos varianto.
• Pridėti nauji Google TTS paketai ir lokalizuoti papildomų kalbų pavadinimai visoje sąsajoje.

Versija 0.8.4 – 2026-07-24

EPUB dokumentų redagavimas
• Sonarpad dabar gali ne tik atidaryti EPUB dokumentus, bet ir juos redaguoti bei vėl išsaugoti EPUB formatu, išlaikant pradinį formatavimą, turinį, išnašas, vaizdus, stilių lenteles, metaduomenis ir vidines nuorodas.
• EPUB pasiekiamas „Išsaugoti kaip“ lange dokumentams, atidarytiems iš EPUB. Išsaugant atnaujinamas tik pakeistas tekstas ir nepažeidžiama knygos struktūra.

Garsinių knygų patikimumas
• Ištaisyta protarpinė problema, kai po penkių nesėkmingų Google TTS bandymų sintezės vienetas buvo tyliai atmetamas ir galutinėje garsinėje knygoje galėjo trūkti dalies teksto.
• Google vienetai dabar kartojami, kol pavyksta arba naudotojas atšaukia. Darbo procesų paleidimas išskirstytas, kad sumažėtų laikini Chrome ir failų konfliktai, o Sonarpad dabar sustoja, užuot išsaugojęs garsinę knygą su trūkstamu segmentu.
• Edge garsinėms knygoms dabar pakartotinai bandoma esant laikiniems tinklo, WebSocket, skirtojo laiko, paslaugos limito ar netinkamo garso atsakymams, kol pavyksta arba naudotojas atšaukia; tai taikoma ir mišriems balsams bei skaidymui pagal laiką. SAPI4 ir SAPI5 išlaiko adaptyvų ribotą atkūrimą; jei segmentas vis tiek nepavyksta, Sonarpad sustoja neišsaugodamas nepilnos garsinės knygos.

Skaitmeninių bibliotekų navigacija
• LibriVox, Internet Archive ir Project Gutenberg paieškos rezultatai dabar naudoja puslapių navigaciją kaip YouTube: „Eiti į ankstesnius rezultatus“ rodoma viršuje, o „Eiti į kitus rezultatus“ — apačioje.
• Ištaisyti LibriVox fokuso perėjimai: atidarius knygą ar skyrių NVDA fokusas nebeperkeliamas į pagrindinį redaktorių prieš atsidarant kitam sąrašui ar leistuvui.
• Pridėta LibriVox fokuso apsauga paieškos ir knygos įkėlimo metu: lokalizuotas įkėlimo dialogas lieka pirmame plane, kol vykdoma užklausa, todėl NVDA fokusas nepabėga į Komandų eilutę, Windows Terminal ar kitą programą.

YouTube grojaraščių atsisiuntimai
• YouTube grojaraščiuose pridėta prieinama kelių elementų pasirinkimo komanda, leidžianti pasirinkti, kuriuos vaizdo įrašus atsisiųsti, nekeičiant esamos „Išsaugoti mediją“ komandos šiuo metu atkuriamam elementui.
• Pasirinkti elementai atsisiunčiami po vieną naudojant formatą ir kokybę, pasirinktus atidarant grojaraštį, gauna sunumeruotus failų pavadinimus, išlaikančius grojaraščio tvarką, ir išsaugomi atskirame aplanke nustatytame Media aplanke.
• Pasirinkimo lange yra komandos Pasirinkti viską ir Atžymėti viską, pranešamas pasirinktų elementų skaičius, galima atšaukti išsaugant jau baigtus failus ir pranešama apie elementus, kurių nepavyko atsisiųsti.
• Grojaraščio įrašai dabar yra įprasti žymimieji langeliai: ekrano skaitytuvai automatiškai praneša kiekvieną pavadinimą, valdiklio vaidmenį ir pažymėjimo būseną, nepridedant pasirinkimo žodžių prie matomo pavadinimo ir nenaudojant priverstinio kalbėjimo.

Versija 0.8.3 – 2026-07-23

Tamsusis režimas
• Pridėtas tamsusis režimas, kurį galima įjungti meniu Rodinys ir kuris išsaugomas naudotojo nuostatose.
• Tamsioji tema taikoma redaktoriui, meniu, antriniams langams ir pagrindiniams valdikliams, o teksto spalvos pritaikytos skaitomumui ir prieinamumui išlaikyti.

Vokiečių kalba
• Vokiečių kalba pridėta kaip visa sąsajos kalba, pasirenkama Parinktyse.
• Naujienos ir RSS, rašybos tikrintuvas, kalendorius ir visos citatos, aukos, vadovas bei pakeitimų žurnalas visiškai pasiekiami vokiečių kalba.

Brazilijos portugalų kalba ir Google Naujienos
• Brazilijos portugalų kalba pridėta kaip visa sąsajos kalba, atskira nuo portugalų (Portugalija) ir pasirenkama Parinktyse.
• Visa sąsaja, kalendoriaus įrašai ir citatos, rašybos tikrintuvas, aukos, vadovas ir pakeitimų žurnalas pasiekiami Brazilijos portugalų kalba.
• Google Naujienos dabar palaiko Brazilijos lokalizaciją, Brazilijos kategorijas ir atskirus numatytuosius Brazilijos RSS šaltinius.
• Susiję tos pačios naujienos Google Naujienų šaltiniai, kai juos pateikia kanalas, medyje rodomi kaip prieinami antriniai elementai.

LibriVox
• Optimizuotos LibriVox paieškos, kad būtų išvengta per daug užklausų paslaugai ir sąsajos užstrigimų. Pašalintas didelių katalogų skenavimas, sumažintas bandymų skaičius ir įvesti trumpesni skirtojo laiko intervalai.

Kalbos sintezė
• Trijų ar daugiau taškų sekos prieš skaitymą dabar normalizuojamos, todėl kai kurie balsai nebetaria „taškas taškas“ ir nebegeneruojami segmentai, sudaryti tik iš skyrybos ženklų.

Susiję Google Naujienų straipsniai
• Kiekvienai naujienai dabar, kai yra, rodomi susiję straipsniai, t. y. kiti straipsniai apie tą pačią istoriją. Norint juos skaityti, tereikia išskleisti pagrindinį straipsnį, kai Sonarpad praneša, kad yra susijusių straipsnių. Nenorintys išskleisti šios dalies gali tiesiog spausti Enter ant pagrindinio straipsnio ir skaityti naujieną kaip įprasta.
• Susiję straipsniai dabar naudoja tą pačią perskaityta / neperskaityta sistemą kaip pagrindiniai straipsniai, įskaitant prieinamus pranešimus, datą ir laiką, išsaugojimo būseną bei išlaikymą po kanalų atnaujinimo ar Sonarpad paleidimo iš naujo.

Garsinės knygos dalių pranešimai
• Garso parinktyse pridėtas kombinuotasis laukelis „Pranešimas kiekvienos dalies pradžioje“. Garsinėms knygoms, suskaidytoms į kelis failus, kiekviena dalis gali prasidėti be pranešimo, knygos pavadinimu, pavadinimu ir dalies numeriu, failo pavadinimu arba failo pavadinimu ir dalies numeriu.

Versija 0.8.2 – 2026-07-17

Skaitmeninės bibliotekos ir garsinės knygos
• Pridėtas Project Gutenberg su paieška pagal pavadinimą arba autorių ir kalbos pasirinkimu.
• Project Gutenberg EPUB knygos atsisiunčiamos į Documents\Sonarpad\Documents; baigus atsisiųsti Sonarpad paklausia, ar knygą iš karto atidaryti redaktoriuje.
• Pridėtas Internet Archive garso kolekcijų paieškai ir klausymui, įskaitant senas radijo laidas, kalbas ir gyvą muziką.
• Pridėtas LibriVox garsinių knygų paieškai pagal pavadinimą arba autorių ir tiesioginiam skyrių atkūrimui tuo pačiu leistuvu, kuris naudojamas tinklalaidėms.
• Visos trys naujos funkcijos pasiekiamos meniu Įrankiai ir, kai įjungtas meniu grupavimas, skiltyje Skaitymas.

Ilgų garso failų transkribavimas
• Ištaisytas ilgų garso failų transkribavimas: garsas dabar automatiškai padalijamas į 15 minučių dalis, transkribuojamas po vieną dalį ir vėl sujungiamas, taip išvengiant klaidų su ilgais įrašais.

YouTube
• Naudingiausi veiksmai, anksčiau pasiekiami tik atidarius YouTube vaizdo įrašą ir meniu Atkūrimas, dabar pasiekiami ir tiesiai to vaizdo įrašo kontekstiniame meniu, pvz., „Transkribuoti dabartinį garsą“, „Kurti garsinį vaizdavimą su DI“ ir „Išsaugoti mediją“.
• Pridėta „Kopijuoti nuorodą“, taip pat pasiekiama su Ctrl+C, kuri nukopijuoja pasirinkto YouTube vaizdo įrašo, grojaraščio ar kanalo URL į iškarpinę.

Versija 0.8.1 – 2026-07-16

Google teksto į kalbą sintezė
• Ištaisyta Google TTS paleidimo problema Windows sistemose, kur vidinio naršyklės serverio priimti ryšiai paveldėdavo neblokuojantį socket režimą, sukeldavo klaidą 10035 ir neleisdavo kalbėti atsisiųstiems balsams.
• Sonarpad dabar laukia, kol Chrome arba Edge WASM variklis bus visiškai įkeltas, prieš balso peržiūrą ar skaitymą su F5, taip išvengiant klaidos „Chrome WASM TTS engine was not loaded“.
• Paslėptoje naršyklėje išjungtas puslapio vertimas ir atvaizdavimo prieinamumas, todėl ji nebegali pranešti „Versti puslapį“ ar trukdyti skaitymo komandoms.
• Skydelyje „Balsai redaktoriuje“ dabar rodoma „Tvarkyti Google balsus...“, kai pasirinktas Google variklis, o uždarius tvarkyklę įdiegtų balsų sąrašas iškart atnaujinamas.
• Priklausomybių įspėjimai, rodomi šalinant Google balsų paketus, dabar lokalizuoti visomis sąsajos kalbomis.

Atnaujinimo patirtis
• Po automatinio atnaujinimo užbaigimo ir pakeitimų žurnalo langas dabar atsidaro po pirminio fokuso grąžinimo į redaktorių ir lieka pirmame plane, užuot pasirodęs tik paspaudus Tab.

PDF dokumentai
• Ištaisyti PDF failai, kurių įterptame tekste buvo NUL ženklų ir įkeliant į redaktorių tekstas būdavo nukertamas ties pirmuoju tokiu ženklu.
• Kai pdf-extract grąžina įterptus NUL ženklus, Sonarpad dabar bando dar kartą su PDFium; likę NUL ženklai pašalinami prieš siunčiant tekstą Windows valdikliams, todėl likusi dokumento dalis išsaugoma.

Meniu prieinamumas
• Pašalintas mnemonikų generavimas vykdymo metu: prieigos klavišai dabar aiškiai įrašyti kiekviename iš 15 sąsajos vertimų, todėl tarp paleidimų išlieka vienodi.
• Peržiūrėti visi stabilūs pagrindinio meniu elementai ir submeniu, įskaitant Atkūrimą, šriftų pasirinkimus, Išsaugoti vaizdą ir Rodyti EPUB rodyklę; trūkstami ar pasikartojantys tos pačios grupės mnemonikai pataisyti tiesiogiai vertimuose.
• Automatiniai testai dabar tik tikrina vertimus ir nepraeina, jei mnemoniko trūksta, jis netinkamas ar pasikartoja; vykdymo metu meniu etiketės niekada nekeičiamos.
• Ypač dideliuose meniu, kur išverstose etiketėse nepakanka skirtingų ženklų, rodomas aiškus skaitinis prieigos klavišas naudojant standartinę Windows formą „(&1)“.

Versija 0.8.0 – 2026-07-15

Internetinis žodynas
• Į internetinį Wiktionary žodyną pridėta vokiečių kalba.
• Vokiečių apibrėžimai ir sinonimai dabar analizuojami pagal vokiško Wiktionary struktūrą, o ne vien pridedant kalbą į pasirinkimo sąrašą.

SAPI5 garsinių knygų patikimumas
• Kuriant SAPI5 garsines knygas išlaikoma iki 12 lygiagrečių darbo procesų, kai pasirinktas balsas pateikia patikimą rezultatą.
• Kiekviena sugeneruota dalis dabar tikrinama pagal failo dydį, numatomą trukmę ir atsargų palyginimą su jai priskirtu tekstu.
• Trūkstamos ar įtartinos dalys automatiškai generuojamos iš naujo palaipsniui mažinant lygiagretumą: 12, 8, 6, 4, 2 ir galiausiai 1 procesas. Kartojamos tik probleminės dalys.
• Patikima procesų riba įsimenama atskirai kiekvienam SAPI5 balsui, nelėtinant balsų, kurie teisingai veikia su 12 procesų.
• Galutinis vientisumo patikrinimas neleidžia Sonarpad tyliai priimti MP3, kuris yra daug trumpesnis už sugeneruotas dalis.
• Išsami diagnostika rašoma į `sapi5_audiobook_diagnostic.log`.
• Kiekvienas SAPI5 sintezės vienetas dabar veikia atskirame paslėptame Sonarpad procese. Jei trečiosios šalies balsas sugenda, užsidaro tik tas procesas, o pagrindinė programa lieka atidaryta.
• To paties garsinės knygos kūrimo metu neužbaigtos dalys iškart kartojamos su kitu mažesniu lygiagretumo lygiu; jau patikrintos dalys išsaugomos.
• Atkūrimas kitą kartą paleidus lieka kaip papildoma apsauga tik tuo atveju, jei nutraukiama pagrindinė programa ar kompiuteris.

SAPI4 garsinių knygų procesai
• Dabar paisoma naudotojo pasirinkto SAPI4 procesų skaičiaus iki techninės 64 ribos; ankstesnė paslėpta 16 riba pašalinta.
• Faktinis skaičius mažinamas tik tada, kai garsinėje knygoje yra mažiau darbo vienetų nei prašyta.
• Jei vienas ar keli SAPI4 tilto procesai nepavyksta, baigtos dalys išsaugomos, o tik nepavykę vienetai automatiškai kartojami palaipsniui mažinant lygiagretumą.
• Sonarpad dabar tikrina SAPI4 tilto išėjimo būseną ir atmeta tuščias ar netinkamas garso dalis, užuot laikęs jas sėkmingomis.

Tarpinio serverio konfigūracija
• Tinklo nustatymuose pridėtas atskiras tarpinio serverio prievado laukas.
• Prievadą dabar galima įvesti nepriklausomai nuo tarpinio serverio adreso, jis tikrinamas nuo 1 iki 65535 ir tinkamai pakeičia bet kurį URL jau esantį prievadą.

Radijo paieška pagal kalbą ir šalį
• Kalbos ir Šalies filtrai dabar atnaujinami visais prieinamais Radio Browser katalogo įrašais, o ne ribojami fiksuotu sąrašu.
• Kalbų pavadinimai dabar atpažįstami net kai Radio Browser juos pateikia kitu raštu, vietiniais pavadinimais, santrumpomis ar kelių kalbų deriniais, ir rodomi išversti į dabartinę sąsajos kalbą. Reikšmės, kurios nėra tikros kalbos, pvz., skaičiai, žanrai, šalys ar bendrinės etiketės, atfiltruojamos.
• Katalogas atnaujinamas fone, o atsarginis sąrašas lieka naudojamas, kai Radio Browser nepasiekiamas.
• Pasikartojantys Radio Browser kalbų įrašai, kurie po vertimo tampa vienodi, dabar sujungiami į vieną kombinuotojo laukelio elementą, kad ekrano skaitytuvams nebūtų tylių žingsnių.

Svarbus patobulinimas: kalbos ir žymeklio judėjimo sinchronizacija
• Kalbos atkūrimo ir žymeklio judėjimo sinchronizacija gerokai pagerinta visiems palaikomiems kalbos sintezės varikliams.
• Kai įjungta „Judinti žymeklį skaitant“, Sonarpad dabar naudoja bendrą progreso sistemą Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 ir OneCore.
• Žymeklis tiksliau seka faktiškai tariamą tekstą, nuosekliau skaidant sakinius ir frazes.
• Gerokai sumažintas per ankstyvas judėjimas, vėlavimai, netolygūs šuoliai ir skirtumai tarp kalbos variklių.
• Teisinga padėtis dabar patikimiau išsaugoma po pauzės, tęsimo, paieškos dokumente ar kalbos variklio pakeitimo.

Atskiri tinklalaidės įrašymo takeliai
• Pridėta „Išsaugoti mikrofono ir sistemos arba programos garsą atskiruose failuose“.
• Kai mikrofonas ir kitas šaltinis įrašomi kartu, Sonarpad gali sukurti vieną failą tik su mikrofonu ir antrą failą su sistemos garsu, viena programa arba pasirinktomis programomis.
• Atskirų šaltinių įrašymas galimas ir MP3, ir WAV formatu.
• Kai parinktis išjungta, Sonarpad ir toliau kuria vieną įprastai sumaišytą failą.
• Atskiri failai palengvina garsumo reguliavimą, triukšmo šalinimą ir vėlesnį tinklalaidžių, interviu bei mokomųjų įrašų redagavimą.

Suplanuoti radijo įrašai
• Radijo įrašus dabar galima suplanuoti iš anksto.
• Kiekvienam įrašui galima pasirinkti stotį, dieną, pradžios valandą ir minutę bei trukmę.
• Galima nustatyti pasirinktinę trukmę nuo 1 iki 1 440 minučių.
• Įrašai gali būti vykdomi vieną kartą, kasdien arba kas savaitę.
• Įrašų lange dabar aiškiau rodomi aktyvūs ir suplanuoti įrašai, planuojama data ir laikas, trukmė bei iki pradžios likęs laikas.
• Suplanuoti įrašai gali naudoti Windows užduočių planuoklę ir pradėti automatiškai net tada, kai Sonarpad dar neatidarytas.

Kalendorius
• Pridėtas visas klaviatūra valdomas kalendorius.
• Galima naršyti ankstesnes ir kitas dienas, greitai grįžti į šiandieną ir peržiūrėti šventes bei minėtinas dienas.
• Pridėtas dienos šventasis ir dienos citata, kuriuos galima skaityti, įgarsinti ar kopijuoti.
• Priminimus galima kurti, redaguoti, trinti, atidėti ir pažymėti kaip atliktus.
• Įspėjimus galima rodyti tiksliai nustatytu laiku arba iš anksto ir naudoti Windows planavimą net kai Sonarpad uždarytas.

Orai
• Pridėta orų prognozės skiltis.
• Galima ieškoti miesto ir greitai vėl atidaryti neseniai peržiūrėtas vietas.
• Pasiekiamos dabartinės sąlygos, temperatūra, mažiausia ir didžiausia reikšmės, drėgmė, kritulių tikimybė ir artimiausių dienų prognozės.
• Temperatūrą galima rodyti Celsijaus, Farenheito laipsniais arba parinkti automatiškai.

Filmai kino teatruose
• Pridėta dabar rodomų ir būsimų kino premjerų skiltis.
• Galima ieškoti pagal pavadinimą, peržiūrėti siužetą, išleidimo datą ir leisti anonsą.

Google teksto į kalbą sintezė
• Pridėtas Google TTS dokumentų skaitymui ir garsinių knygų kūrimui.
• Pridėta balsų tvarkyklė, skirta balsams išvardyti, filtruoti pagal kalbą, atsisiųsti ir pašalinti nebereikalingus balsus.
• Galima reguliuoti greitį, garsumą ir tono aukštį.
• Google Natural balso tono aukštį tiesiogiai taiko variklis, kad rezultatas būtų natūralesnis ir stabilesnis.
• Pagerintas Google TTS reagavimas ir patikimumas, sintezės laiko ribas pritaikant pasirinktam kalbėjimo greičiui.
• Sumažintas nereikalingas laukimas, kai variklis neatsako, ir pagerintas klaidų bei nutraukimų tvarkymas.
• Diagnostikos registravimas stabilesnis atliekant kelias operacijas vienu metu.

EPUB turinys
• Sonarpad dabar atpažįsta EPUB knygose įterptą turinį.
• Apie jo buvimą pranešama ir jį galima atidaryti iš meniu Rodinys.
• Skyriai ir poskyriai rodomi hierarchiškai.
• Paspaudus Enter iškart pereinama į pasirinktą knygos vietą.

Naujienų ir RSS šaltiniai
• Naujienų skiltis išplėsta naujais paieškos ir organizavimo įrankiais.
• Pridėtas naujienų kalbos pasirinkimas.
• Galima ieškoti RSS šaltiniuose ir skaityti savo miesto naujienas.
• Bendruomenės RSS šaltinius galima naršyti, pridėti į asmeninę kolekciją ir pateikti Sonarpad bendruomenei.

Tinklalaidžių įrašymas
• Galima įrašyti tik mikrofoną, visą sistemos garsą, vieną programą, kelias pasirinktas programas arba mikrofoną ir programas kartu.
• Galima pasirinkti mikrofono įrenginį ir garso šaltinį, atskirai reguliuoti šaltinių garsumą ir realiu laiku stebėti lygius.
• Pridėta pauzė ir tęsimas, MP3 arba WAV išvestis, MP3 bitų spartos ir paskirties aplanko pasirinkimas.
• Įrašymo metu kompiuterį galima neleisti užmigdyti.
• Atskiri failai gauna skirtingus pavadinimus, kad mikrofono takelį būtų galima iškart atskirti nuo sistemos ar programos garso.

Radijas
• Radijo skiltis iš esmės pertvarkyta.
• Stočių galima ieškoti pagal pavadinimą ar laisvą tekstą, kalbą, šalį, miestą, muzikos žanrą ar kategoriją.
• Pagerintas parankinių valdymas ir visus filtrus galima greitai atstatyti.
• Stotis galima pateikti Sonarpad bendruomenei.
• Pridėtas tiesioginis įrašymas, „Įrašyti ir leisti“, įrašų sąrašas bei įrašų trynimas ir valdymas.
• Radijo įrašai saugomi atskirame aplanke pagrindiniame įrašų kataloge.

Medijos atkūrimas
• Gerokai pagerintas medijos leistuvo stabilumas.
• Ištaisyta problema, galėjusi užblokuoti mpv, ir padidintas ryšio su leistuvu patikimumas.
• Pagerintas įvairių tipų medijos failų atidarymas.
• Sonarpad dabar prisimena atkūrimo garsumą.
• Pagerintas srautų ir įrašų tvarkymas.
• Ištaisyti failai, atidaromi iš Windows dukart spustelint arba per „Atidaryti naudojant“.

PDF dokumentai
• Pridėtas formų laukų atpažinimas PDF dokumentuose.
• Sonarpad gali rasti pildomus laukus, pateikti juos prieinamu tekstiniu pavidalu, leisti redaguoti reikšmes ir išsaugoti įvestus duomenis atgal į PDF.
• Ištaisytas žymeklio padėties skaičiavimas kalbėjimo metu, ypač dokumentuose su kelių baitų simboliais ar sudėtingomis struktūromis.
• Nauja bendra sinchronizavimo sistema dar labiau pagerina žymeklio judėjimą su kiekvienu kalbos sintezės varikliu.

Prieinamumas ir klaviatūros komandos
• Pagerintos standartinės redagavimo komandos visoje programoje.
• Kopijuoti, iškirpti, įklijuoti, pasirinkti viską, anuliuoti ir pakartoti dabar tinkamai siunčiami fokusuotam laukui, įskaitant antrinius langus ir dialogus.
• Ištaisyta problema, dėl kurios Brailio ekranai galėjo neatnaujinti informacijos.
• Pagerintas fokuso tvarkymas antriniuose languose.
• Ištaisytas kalbos pasirinkimas Wikipedia lange.
• Pridėta parinktis grupuoti Įrankių meniu funkcijas pagal kategorijas.
• Pridėti konfigūruojami veiksmai greitai atidaryti Kalendorių, Orus ir Filmus kino teatruose.
• Pagerintas pakeitimų žurnalo rodymas po atnaujinimo.

Garsinės knygos
• Pagerintas garsinių knygų kūrimas, kai atidaryti dialogai ar kiti modaliniai langai.
• Progreso tvarkymas tapo atsparesnis ir ignoruoja pasenusius garso atnaujinimus, todėl mažiau užstrigimų, neteisingų pranešimų ir nereaguojančių langų.
• Google TTS taip pat galima naudoti garsinėms knygoms kurti, valdant greitį, garsumą ir tono aukštį.

Dirbtinis intelektas
• Numatytasis Gemini modelis atnaujintas į `gemini-3.5-flash`.

Bendrieji pataisymai
• Ištaisyti keli mpv atkūrimo užstrigimai.
• Ištaisytas kai kurių garso ir vaizdo failų atidarymas.
• Pagerintos medijos leistuvui siunčiamos komandos.
• Ištaisytas žymeklio atkūrimas kalbos atkūrimo metu.
• Ištaisyti spartieji klavišai pagalbinių langų teksto laukuose.
• Pagerintas garsinių knygų kūrimo stabilumas.
• Ištaisyti išoriškai per Windows atidaromi failai.
• Pagerintas bendras medijos, RSS, radijo ir EPUB tvarkymas.

Versija 0.7.1 – 2026-05-13

Naujos funkcijos ir patobulinimai
• Sukurta oficiali svetainė sonarpad.com — nauja vieta naujienoms, naujausios programos versijos atsisiuntimui, lankytojų komentarams ir ateityje visoms Sonarpad tinklalaidėms. Žinyno meniu dabar yra ir „Aplankyti sonarpad.com“.
• Ištaisyta problema, kai failai su diakritiniais ar specialiaisiais simboliais sukeldavo klaidą paleidžiant balso transkribavimą.
• Nuo šiol Rodinio meniu tokie elementai kaip Eilučių laužymas ir Rodyti vaizdą atkūrimo metu visada rodo teisingą įjungtą ar išjungtą būseną.
• Pagerinta YouTube paieška, leidžianti su Esc grįžti į ankstesnį puslapį ar ekraną.
• Pridėtas išankstinis patikrinimas, ar vaizdo įrašą galima leisti. Sonarpad dabar gali leisti ir kaip miksus pažymėtus vaizdo įrašus ar grojaraščius, kurių anksčiau leisti nepavykdavo.
• Pagerintas automatinis žymelių valdymas. Anksčiau išjungus Automatinės žymelės anksčiau sukurtos žymelės likdavo aktyvios; dabar jos teisingai ignoruojamos iki funkciją įjungiant vėl. Pasiekus medijos failo pabaigą žymelė automatiškai pašalinama.
• Pagerintas žymų tvarkymas įjungus dialogus. Sonarpad dabar teisingai valdo abi funkcijas ir leidžia įterpti žymas net kai dialogų parinktis įjungta.
• Pagerinti balso nustatymai aiškiai atskiriant variklius. Balso profiliai dabar teisingai išlaiko kiekvieno variklio — Edge, SAPI5 ir SAPI4 — parametrus.
• Pridėta pauzės įterpimo žyma, pasiekiama parinktyse arba balso skydelyje paspaudus Tab iš redaktoriaus. Galima rinktis 250 ms, 500 ms, 1 sekundę, 2 sekundes arba savą trukmę.
• Ištaisytas elgesys leidžiant YouTube vaizdo įrašą ir paleidžiant transkribavimą. Grįžus su Alt+Tab fokusas dabar teisingai būna ant aktyvios transkripcijos mygtuko Atšaukti.
• Baigtos transkripcijos dabar išsaugomos automatiškai.
• Pagerintas Wikipedia importas. Galima skaityti tik vieną skiltį ir su Esc grįžti iš straipsnio į paiešką arba importuoti visą straipsnį. Taip pat galima pasirinkti naudojamos Wikipedia kalbą.
• Pridėta pasaulinio radijo skiltis, kur stotis galima ieškoti pagal šalį, kalbą ir žanrą. Vietines radijo stotis galima pridėti į Sonarpad duomenų bazę, kad jų galėtų klausytis kiti, ir pridėti į parankinius.
• Pridėta maršrutų skiltis, skirta apskaičiuoti keliones pėsčiomis, dviračiu, automobiliu ar neįgaliojo vežimėliu. Galima pasirinkti trumpiausią ar greičiausią maršrutą ir ar rodyti kertamas savivaldybes. Importavus maršrutą vaizdinį žemėlapį galima išsaugoti per Failas > Išsaugoti vaizdą.
• Failo meniu pridėta Spausdinti. TXT failus Sonarpad spausdina savo sistema, o DOCX, PDF ir panašiems naudoja susietą programą, kad kiek įmanoma išlaikytų originalų išdėstymą.
• Kiekvienam dokumentui pridėta vertimo paslauga, pasiekiama redaktoriaus kontekstiniame meniu. Nemokamai galima naudoti DeepL ir Google Translate be API rakto, o įvedus Gemini API raktą — versti su Gemini.
• Vertimo meniu galima pasirinkti tikslinę kalbą. Meniu automatiškai persirikiuoja: jei pirmiausia pasirenkama anglų, po to prancūzų ir italų, šios trys kalbos rodomos meniu viršuje.
• Įvedus Gemini API raktą taip pat pasiekiama kontekstinio meniu funkcija Apibendrinti tekstą, skirta bet kuriam straipsniui sutrumpinti.
• Atkūrimo meniu pridėta komanda, rodoma leidžiant medijos failą, skirta dabartinei medijai padalyti. Ji veikia su MP3, MP4 ir kitais formatais, dalijant pagal dalių skaičių arba kiekvienos dalies trukmę.

Versija 0.7.0 – 2026-04-25

Kas naujo
• Pridėtas mpv leistuvo palaikymas srautiniam atkūrimui. YouTube ir palaikomų svetainių vaizdo įrašai dabar leidžiami iš karto; jei naudotojas nori juos pasilikti, jie atsisiunčiami kaip anksčiau. Transkribuojant srautinį turinį jis pirmiausia atsisiunčiamas, tada transkribuojamas. mpv taip pat naudojamas vietiniams vaizdo įrašams ir subtitrams, todėl pagerėjo suderinamumas su daugeliu anksčiau nevisiškai palaikytų formatų.
• Pagerintas sistemos garso įrašymas tinklalaidėms: dabar galima įrašyti visą sistemos garsą, vieną programą ar kelias programas vienu metu. Mikrofoną vis tiek galima atskirai įjungti arba išjungti.
• Pridėta hindi kalba. Išversta sąsaja, pridėti RSS kanalai, pakeitimų žurnalas ir Sonarpad vadovas.
• Redaktoriaus kortelėje pridėta parinktis naudojant rodykles aukštyn ir žemyn visada perkelti žymeklį į eilutės pradžią.
• Meniu „Konvertuoti garsą“ pridėtas M4B.

Pataisymai
• Ištaisytas `F10`, kad skaitant tekstą vėl perjungtų į kitą parankinį balsą.
• Vykstant tinklalaidės įrašymui kito dokumento uždarymas nebeuždaro aktyvaus įrašo.
• YouTube komentaruose, atidarytuose iš „Leisti srautinį garsą...“, Sonarpad iš pradžių įkelia tik pirmus 50 pagrindinių komentarų su visais jų atsakymais ir prideda paskutinį elementą visiems komentarams įkelti pagal poreikį.
• Žymelės dabar rodomos ir tvarkomos pagal padėtį tiek teksto dokumentuose, tiek medijos failuose, o ne pagal sukūrimo laiką. Žymelė toje pačioje vietoje nebepridedama pakartotinai.
• Žymelių meniu pridėta automatinio žymelių valdymo parinktis. Uždarius leidžiamą vietinį ar srautinį failą Sonarpad automatiškai išsaugo pasiektą padėtį ir kitą kartą tęsia nuo jos. Tas pats galioja tekstams: uždarant įsimenama žymeklio padėtis, o pradėjus skaitymą išsaugomas paskutinis perskaitytas sakinys ir kitą kartą tęsiama nuo jo.
• Rodinio meniu pridėtas vaizdo rodymo vietiniams ar srautiniams failams jungiklis. Vaizdas rodomas padidintame lange, kuriame valdikliai slepiami, kol nepaspaudžiamas Alt ar pelė neperkeliama į viršų. Tai padidina turinį ir pagerina naudojimą silpnaregiams.

Versija 0.6.9 – 2026-04-08

Pataisymai
• Pagerinta Paieška failuose: atidarius Naršyti aplanką fokusas iškart pereina į aplankų sąrašą; Enter ant rezultato nebesugadina klaviatūros komandų; Esc grąžina į anksčiau pasirinktą rezultatą; grįžus su Alt+Tab fokusas eina į paieškos lauką arba į rezultatų sąrašą, jei jis atidarytas.
• F5 visada pradėdavo skaityti nuo pradžios. Dabar skaitymas prasideda nuo esamos žymeklio vietos, o `Shift+F5` ir `Ctrl+F5` lieka ankstesnio ir kito sakinio navigacijai.
• Po Eiti į eilutę paspaudus Esc fokusas galėjo išeiti iš Sonarpad. Dabar jis teisingai grąžinamas į redaktorių.
• `Eilučių laužymas` dabar iškart pritaikomas jau atidarytiems dokumentams, o ne tik vėl atidarius failą.

Versija 0.6.8 – 2026-04-07

Kas naujo
• Atkūrimo meniu pridėta nauja komanda bet kuriam garso ar vaizdo failui transkribuoti su Whisper. Parinktyse atsirado skiltis „DI ir transkribavimas“, kur galima pasirinkti modelį, įjungti pasirinktinį CUDA palaikymą NVIDIA vaizdo plokštėms, išlaikyti originalo kalbą ir įjungti arba išjungti laiko žymas.
• Atkūrimo meniu pridėta `Transkribuoti dabartinį aplanką`: visi palaikomi garso failai iš šiuo metu atidarytos medijos aplanko transkribuojami į vieną sujungtą dokumentą, rodant atskirą progresą, dabartinio failo būseną ir suteikiant galimybę atšaukti. Funkciją galima paleisti ir su `Alt+Shift+C`.
• Pridėtas neprisijungus veikiantis diktavimas balsu, naudojantis tą pačią darbo eigą kaip garso transkribavimas. Pagal numatytuosius nustatymus `Ctrl+Shift+Space` pradeda diktavimą, o tas pats derinys jį sustabdo; spartųjį klavišą galima keisti Parinktyse. Nuo antro paleidimo diktavimas greitesnis, nes variklis lieka paruoštas atmintyje; kompiuteriuose su mažiau nei 4 GB RAM šis išankstinis įkėlimas ir pakartotinis naudojimas automatiškai išjungiami.
• Pridėta nauja Redaktoriaus parinktis, pagal numatytuosius nustatymus išjungta, leidžianti `Esc` uždaryti redaktoriaus langą.
• Tinklalaidžių paieška dabar pagal numatytuosius nustatymus naudoja `iTunes + Spreaker`, pašalindama dublikatus, kai ta pati tinklalaidė randama abiejose platformose.
• Pagerintas Apple tinklalaidžių naršymas ir paieška: paieška, kategorijų naršymas ir populiariausios tinklalaidės pagal kategoriją naudoja pasirinktą tinklalaidžių katalogo šalį. Parinktys > RSS / Tinklalaidės galima palikti `Automatinė`, kad būtų naudojama sistemos šalis, arba pasirinkti kitą rankiniu būdu.
• Padidinta Apple tinklalaidžių kategorijų rezultatų riba. Pirmą kartą vis dar įkeliama 50 rezultatų; pasirinkus `Įkelti daugiau rezultatų` Sonarpad įkelia iki 200 rezultatų — Apple ribos — ir leidžia sklandžiai naršyti kitus puslapius.
• Sonarpad dabar pasiekiamas ir Mac su dalimi funkcijų. Projekto nuoroda: https://github.com/Ambro86/Sonarpad-Mac

Patobulinimai
• Tinklalaidžių katalogui pridėta daugiau nei 50 pasirenkamų šalių, todėl galima naudoti daug daugiau nacionalinių katalogų.
• „Leisti srautinį garsą...“ dabar gali ieškoti YouTube pagal bet kokią tekstinę užklausą arba priimti YouTube kanalo ar grojaraščio nuorodą ir parodyti jos rezultatus.
• Pagerintas rezultatų rodymas „Leisti srautinį garsą...“: YouTube įrašai aiškiau pateikia pavadinimą, trukmę, kanalą ir peržiūrų skaičių.
• „Leisti srautinį garsą...“ dabar palaiko YouTube komentarus: juos galima atidaryti iš kontekstinio meniu, skaityti atsakymus ir išskleisti komentarų gijas rodykle dešinėn.
• „Leisti srautinį garsą...“ pridėti YouTube parankiniai kanalams ir grojaraščiams. Juos galima pridėti iš rezultatų kontekstinio meniu, atidaryti iš Parankinių sąrašo, pasiekiamo su Tab po YouTube URL / užklausos lauko, ir vėliau pašalinti iš to paties sąrašo. YouTube paieškos rezultatuose kontekstinis meniu galimas tik kanalams ir grojaraščiams.
• „Leisti srautinį garsą...“ dabar gali paprašyti prisijungimo duomenų, kai srautinė svetainė reikalauja autentifikacijos. Duomenis galima įvesti, išsaugoti svetainei ir vėliau tvarkyti Parinktys > Garsas.
• Pagerintas fokusas „Leisti srautinį garsą...“ metu, kad progreso langas būtų stabilesnis atsisiuntimo ir konvertavimo metu.
• Balso meniu pridėti du skaitymo navigacijos veiksmai: `Ankstesnis sakinys` ir `Kitas sakinys`, su konfigūruojamais sparčiaisiais klavišais.
• Numatytasis `Vykdyti failą su interpretatoriumi` spartusis klavišas dabar yra `Ctrl+Shift+F5`, todėl `Shift+F5` pagal numatytuosius nustatymus gali būti naudojamas `Ankstesniam sakiniui`.
• Parinktys > Balsas pridėti balso profiliai: juos galima pridėti, pervadinti ir šalinti.
• Parinktys > Garsas išplėsti atkūrimo atsukimo intervalai — nuo 1 sekundės iki 2 valandų.
• Pridėtas rusų vertimas, ačiū Dmitriy.
• Parinktys > Garsas pridėtas garsinės knygos dalių pavadinimo formatas: `Pavadinimas + numeris`, `Tik numeris` arba `Numeris + pavadinimas`.
• Pridėti RSS parankiniai straipsniai: iš straipsnio kontekstinio meniu juos galima pridėti į specialų Parankinių kanalą.
• Parankinių RSS kanalą galima pašalinti; pridėjus naują mėgstamą straipsnį jis automatiškai sukuriamas iš naujo.
• Pridėti RSS spartieji klavišai kanalams perkelti aukštyn / žemyn: `Ctrl+Shift+Rodyklė aukštyn` ir `Ctrl+Shift+Rodyklė žemyn`.
• RSS lange pridėta integruota straipsnio peržiūra, kurią galima greitai pasiekti su Tab prieš atidarant visą straipsnį redaktoriuje.
• RSS kanalų gale, kai yra daugiau įrašų, pridėtas aiškus elementas „Įkelti daugiau naujienų“; Enter įkelia kitą paketą ir perkelia fokusą į pirmą naujai įkeltą straipsnį.
• Balso žodyne pridedant ar redaguojant pakeitimą dabar yra „Skirti didžiąsias ir mažąsias raides“, kad kiekvienas pakeitimas galėtų paisyti arba ignoruoti raidžių registrą.

Klaidų taisymai
• „Leisti srautinį garsą...“ dabar paiso Parinktyse nustatytos tinklalaidžių talpyklos ribos; ta pati riba taikoma ir garsinių aprašymų atkūrimui.
• Ištaisytas Wikipedia importas, kad puslapių citatų blokai būtų importuojami teisingai.
• Pagerintas WordPress puslapių analizatorius, kai būdavo praleidžiami sąrašo elementai ar kai kurios skirsnių antraštės.
• „Eiti į eilutę“ dabar iš anksto įrašo dabartinės eilutės numerį.
• Ištaisytas tinklalaidžių ir RSS OPML eksportas, kad eksportuotus failus priimtų iTunes.
• Pridėti lokalizuoti patvirtinimo pranešimai apie sėkmingą RSS ir tinklalaidžių OPML importą bei eksportą.
• Ištaisyta klaida „Leisti srautinį garsą...“, kai įvedus paiešką ir pasirinkus YouTube kanalą programa galėjo atrodyti užstrigusi, užuot atidariusi kanalo vaizdo įrašus.
• Ištaisyta, kad atidarytų failų sąrašas buvo rodomas Žinyno, o ne Lango meniu.
• Ištaisytas srautinio atkūrimo atvejis, kai atkūrimas prasidėdavo, bet „Atsisiunčiamas srautas“ dialogas likdavo atidarytas, jei failas jau atitiko tikslinį formatą.
• Ištaisytas MP3 srauto konvertavimas: jei srautas jau MP3, o naudotojas pasirenka konkretų MP3 bitų dažnį, pvz., 128 kbps, Sonarpad dabar perkoduoja į pasirinktą dažnį, o ne praleidžia konvertavimą.
• Ištaisyti medijos transkripcijos dokumentai: uždarant dabar klausiama, ar išsaugoti, o siūlomas failo vardas naudoja transkribuotos medijos failo vardą, ne pirmą teksto eilutę.
• Ištaisytas `Alt+Shift+L`: atkūrimo metu teisingai atidaromas skyrių sąrašas.
• Ištaisytas `Alt+Shift+T`: dabar teisingai paleidžia „Transkribuoti dabartinį garsą“, o ne atidaro Įrankių meniu.
• Ištaisytas atkūrimo sustabdymas: `.` dabar elgiasi kaip Sustabdyti ir sustabdo tik dabartinį takelį, neišeidamas iš leistuvo / epizodo.
• Ištaisytas medijos iš Naujausi failai išsaugojimo punktas: kai failas yra vietinėje Sonarpad talpykloje, lokalizuota išsaugojimo komanda dabar rodoma ir ten.
• Jei transkribavimas pradedamas jau grojant garsui, Sonarpad pirmiausia automatiškai jį pristabdo.
• Ištaisyta klaida, kai Wikipedia straipsnis galėjo būti importuotas, bet jo tekstas nepasirodydavo ekrane.
• Pridėtas įterptų tinklalaidžių skyrių palaikymas vietiniuose medijos failuose, pvz., MP3 skyrių metaduomenyse. Jei kanalų / URL skyrių nėra, Sonarpad fone įkelia skyrius iš atsisiųsto failo, todėl atkūrimas prasideda iškart, o skyrių duomenys pritaikomi kai tik paruošti.
• Ištaisytas skyrių įkėlimas atsisiųstiems tinklalaidžių epizodams, atidarytiems kaip įprasti vietiniai medijos failai: įterpti skyriai dabar pasiekiami ir ten.
• Ištaigytas MP3 garsinių knygų užbaigimas SAPI4 ir SAPI5: galutinis failas tinkamai užbaigiamas, kad po ilgų eksportų nebūtų nepilnas ar pažeidžiamas.
• Visuose garsinių knygų kūrimo režimuose pridėta aiški galutinio užbaigimo progreso juosta: po kūrimo Sonarpad praneša ir rodo atskirą užbaigimo etapą su progresu.
• Ištaisyti dialogo balsų nustatymai: greitis, tonas ir garsumas dabar teisingai taikomi abiem dialogo balsams.
• Pagerintas japoniškų `.txt` failų koduotės aptikimas: pridėtas saugus Shift_JIS/CP932 atsarginis variantas sugadinto teksto atvejams, išlaikant esamą UTF, diakritikos ir kinų kalbos elgesį.
• Vidinis saugumo pertvarkymas: kur įmanoma, funkcijos pakeistos saugiomis realizacijomis ir gerokai sumažintas unsafe kodo eilučių skaičius.

Versija 0.6.7 – 2026-03-02
Patobulinimai
• Programa dabar gali masiškai vykdyti „Pakeisti viską“ dideliuose failuose, kuriuose yra labai daug pakeitimų.
• Atnaujintas lenkų vertimas, ačiū DJ Graco.
• Pridėtas lietuvių vertimas.
• Pridėtas kinų vertimas.
• Dažni beta leidimai dabar bus skelbiami projekto Releases skiltyje, kad pakeitimus būtų galima išbandyti prieš kitą stabilią versiją.
• Pridėtas `Ctrl+.` daugtaškio ženklui (…) įterpti.
• Pagerintas tinklalaidžių skyrių palaikymas: navigacija patikimiau veikia ir tiesioginiams / srautiniams epizodams, kai skyriai neįterpti MP3, naudojant kanalo / URL metaduomenis. Pridėti `Ctrl+Alt+PageUp` ankstesniam ir `Ctrl+Alt+PageDown` kitam skyriui.
• Sonarpad išvesties aplankai pertvarkyti po `Documents\Sonarpad`: failai saugomi `audiobooks`, `documents`, `recordings` ir `media`, automatiškai perkeliant iš senų kelių.
• Pagerintas labai didelių teksto failų, įskaitant 60 MB, palaikymas: sklandesnis atidarymas ir navigacija eilutėmis, ypač su ekrano skaitytuvais.
• Atnaujinti visų kalbų vadovai ir lokalizacijos ištekliai, įskaitant aukų tekstus ir NSIS diegimo vertimus: naujos supaprastintos kinų ir lietuvių diegimo eilutės bei užbaigtas ukrainietiškas diegimo vertimas.
• Internetinėms funkcijoms pridėtas globalus HTTP/HTTPS ir SOCKS5/SOCKS5H tarpinis serveris su patikra išsaugant Parinktis; netinkami serveriai įspėjami ir automatiškai pašalinami.
• Įrankiuose pridėta „Leisti srautinį garsą...“: galima įklijuoti YouTube ar tiesioginės medijos URL, pasirinkti išvesties formatą ir kokybės / bitų spartos profilį, įskaitant originalią MP3 ir MP4 kokybę, ir leisti tiesiai Sonarpad leistuve.
• Pridėtas sistemos Play/Pause medijos klavišo palaikymas: jis valdo ir medijos atkūrimą, ir teksto skaitymo pauzę / tęsimą, pirmenybę teikdamas medijai.
• Failas > Naujausi failai pridėta „Išvalyti naujausius failus“.
• Išplėsti bitų spartos pasirinkimai Konvertuoti garsą ir tinklalaidžių įrašyme: pridėti 64/96 kbps ir MP3 iki 320 kbps, kartu atnaujinant patikrą ir kodavimą.
• Garsinių knygų skaidymo pagal laiką pasirinkimai išplėsti iki 60 minučių.
• Pagerintas skaidymas pagal dalis: dalių skaičių galima įvesti rankiniu būdu nuo 1 iki 100.
• Pridėta Rodinys > Tik skaitymo režimas, saugantis tekstą nuo netyčinių pakeitimų, bet išlaikantis visą skaitymą ir navigaciją.
• Programos atnaujinimo metu pridėta prieinama progreso juosta, kad ekrano skaitytuvai galėtų sekti atsisiuntimą realiu laiku.
• Pagrindiniame lange pridėta tyli būsenos juosta, rodanti simbolius, žodžius, eilutę ir stulpelį, netrikdant NVDA fokuso.
• Rodinio meniu pridėtas greitas Eilučių laužymo jungiklis.
• Redaguoti > Tekstas pridėtos įtraukos ir įtraukos mažinimo komandos su `Ctrl+Shift+.` ir `Ctrl+Shift+,`, nes įjungus „Rodyti balsus redaktoriuje“ Tab skirtas balso skydelio navigacijai.
• RSS straipsniuose ir tinklalaidžių epizoduose pridėta lokalizuota data / laikas pagal dabartinę sąsajos kalbą.
• RSS kontekstiniame meniu pridėtas pasirinkto straipsnio bendrinimas el. paštu.
• Parinktys > RSS ir tinklalaidės pridėtos detalios trynimo patvirtinimo parinktys: RSS (kanalas / straipsnis / abu / nieko) ir tinklalaidės (tinklalaidė / epizodas / abu / nieko).
• Pridėtas konfigūruojamas greitas RSS kopijavimas su Ctrl+C: pavadinimas, URL, straipsnio turinys arba viskas kartu.
• Suvienodintas RSS šaltinio kūrimas: „Pridėti šaltinį“ priima tiesioginį kanalo URL arba raktažodį, iš kurio automatiškai kuriamas Google News RSS.
• Ctrl+A dabar praneša apie pasirinkimo pabaigą aiškesniam ekrano skaitytuvo grįžtamajam ryšiui.
• Pridėtas Shift+F3 „Rasti ankstesnį“ greta F3 „Rasti kitą“.
• Pagerinti pakeitimo pranešimai su taisyklingomis vienaskaitos / daugiskaitos formomis.
• Žodyno lange pridėtas paieškos kalbos pasirinkimas: pagal numatytuosius nustatymus Automatinė (sąsajos kalba) arba rankinis pasirinkimas.
• Parinktyse pridėta Sparčiųjų klavišų kortelė su konfliktų aptikimu.
• Pradinis komandų eilutės palaikymas: `-h`/`--help` rodo naudojimo informaciją, `--version` — programos versiją.
• Rankinio greičio ir tono valdymo laukai dabar naudoja 100 centruotą skalę, kur 100 yra normali reikšmė.
• Pagerintas Microsoft balsų pasirinkimas Parinktys > Balsas ir redaktoriaus balso skydelyje: pridėtas lokalizuotas kalbos filtras; tik daugiakalbių balsų režime lieka vienas nesugrupuotas sąrašas ir kalbos laukas slepiamas.
• Parinktys > Balsas pridėta dialogo balsų konfigūracija su visa Tab navigacija, tuo pačiu modeliu kaip pagrindinėje sąsajoje ir pasirenkamu antru dialogo balsu. Taisyklės saugomos `.ini`, todėl dokumento tekstas nekeičiamas.
• Pagerinta Anuliuoti etiketė: dabar rodoma, koks veiksmas bus atšauktas, pvz., teksto redagavimas, citavimas arba balso žymos įterpimas, o kai nėra ką anuliuoti punktas išjungtas.

Klaidų taisymai
• Ištaisytas RTF atidarymas: `.rtf` dabar analizuojami ir rodomi kaip skaitomas paprastas tekstas, o ne žalia RTF žymė, pvz., `{\rtf1...}`.
• Ištaisytas GB18030/GBK koduotės kinų teksto failų atidarymas, kad būtų išvengta mojibake.
• Pagerintas M4B garsinių knygų kūrimas su skyrių metaduomenimis ir žymekliais; ištaisytas aukšto tono / greičio „burunduko“ atkūrimas.
• Ištaisyta garsinių knygų išsaugojimo dialogo bitų spartos sąsaja: pašalintos kietai įrašytos itališkos etiketės ir pridėta 64 kbps.
• Ištaisytas Išsaugoti viską (`Ctrl+Shift+S`): visi pakeisti dokumentai patikimai aptinkami, įskaitant naujus / neišsaugotus, ir kiekvienas išsaugomas arba atidaromas Išsaugoti kaip.
• Ištaisyta Google News RSS straipsnių tvarka: kai yra datos, rodomi naujausi pirmiausia.
• Ištaisyta NVDA etikečių sąsaja žodyno lange: paieškos laukas ir kalbos pasirinkimas praneša teisingas etiketes.
• Ištaisyta RSS / tinklalaidės Ypatybių lango klaviatūra: Tab/Shift+Tab pasiekia OK, Enter aktyvina OK, Esc saugiai uždaro, fokusas grįžta į sąrašą.
• Ištaisyta RSS / tinklalaidžių anuliavimo istorija: Ctrl+Z palaiko kelių lygių pašalinimų anuliavimą.
• Pagerintas RSS / tinklalaidžių pašalinimo grįžtamasis ryšys su aiškiais būsenos pranešimais.
• Pagerintas fokusas po trynimo / anuliavimo: RSS prireikus patikimai fokusuojamas pirmas kanalas ir vengiama pasikartojančių ekrano skaitytuvo pranešimų.

Versija 0.6.6 – 2026-02-13
Patobulinimai
• Redagavimo meniu pridėta „Automatiškai formatuoti TTS“, greitai paruošianti tekstą kalbai: pašalina Markdown / citatas ir sutvarko laužytas eilutes.
• Pagerintas balso žymų įterpimas: pažymėtam tekstui žymos dabar teisingai taikomos ir vienos, ir kelių eilučių pažymėjimui.
• Garso nustatymuose pridėta numatytojo garsinių knygų išsaugojimo aplanko parinktis (numatyta: Documents\\Sonarpad Audiobooks).
• Garsinės knygos išsaugojimo lange, kai įjungtas skaidymas, pridėta pagal numatymą įjungta parinktis sukurti atskirą poaplankį dalims, kad rezultatai būtų tvarkingesni.
• Eksportuojant garsines knygas MP3 dabar išsaugomas stereo režimu ir naudojama vartotojo pasirinkta bitų sparta Edge, SAPI5 ir SAPI4 balsams.
• Per tarpinį modulį pridėtas 32 bitų SAPI5 balsų palaikymas, todėl Sonarpad galima naudoti ir balsus, esančius tik 32 bitų varikliuose.
• Balso funkcijos pertvarkytos į atskirą meniu „Balsas ir garsas“, o funkcija „Konvertuoti garsą“ pridėta / paaiškinta kaip būdas konvertuoti bet kurį palaikomą medijos failą į MP3, AAC, OGG, Opus, FLAC, WAV ir AIFF.
• Pridėta galimybė pašalinti atskirus RSS straipsnius ir tinklalaidžių epizodus (Delete + kontekstinis meniu su patvirtinimu), nepašalinant viso RSS / tinklalaidės šaltinio, taip pat anuliuoti paskutinį pašalinimą (atskirą straipsnį / epizodą arba visą šaltinį).
• RSS lange pridėtas RSS kanalų eksportas į OPML, kad esamus RSS šaltinius būtų lengva išsaugoti ir vėliau importuoti.
• RSS lange pridėta „Ieškoti RSS pagal raktažodį“: įvedus raktažodį automatiškai sukuriamas Google News RSS URL ir atidaromas iš anksto užpildytas šaltinio pridėjimo langas, todėl raktažodžio kanalą galima sukurti vienu veiksmu.
• Pridėtas serbų kalbos vertimas, ačiū Mila Kuran.
• Pridėtas ukrainiečių kalbos vertimas, ačiū Ivan Shtefuriak.
• Pridėtas kelių medijos failų atidarymas: pasirinkus / atidarius kelis medijos failus dabar sukuriama atkūrimo eilė, o ne pakeičiamas dabartinis failas.
• Pridėti skirtingo dydžio persukimo spartieji klavišai atkūrimo metu: kai pagrindinis šuolis yra 1 minutė, Kairė / Dešinė peršoka 60 s, Shift+Kairė / Dešinė – 20 s, o Ctrl+Kairė / Dešinė – 3 minutes.
• Leistuve pridėti ankstesnio / kito takelio spartieji klavišai: Ctrl+PageUp ir Ctrl+PageDown.
• Pridėta „Atstatyti garsumą“, o atstatymo veiksmai sugrupuoti į atskirą „Atstatyti“ pomeniu skiltyje Atkūrimas kartu su „Atstatyti greitį“ ir „Atstatyti toną“.
• Pagerintas diegiklis: setup.exe dabar leidžia pasirinkti susieti visus palaikomus failų tipus arba rankiniu būdu pasirinkti plėtinius; MSI funkcijų medyje dabar pateikia susiejimo parinktis kiekvienam plėtiniui (pagal numatymą visi lieka įjungti).
• Pridėtas naujas meniu „Langas“ su „Atidaryti dokumentai...“, leidžiantis greitai persijungti į bet kurį šiuo metu atidarytą failą.
• Atnaujinta Rodymas > Šriftas: senas pasirinkimo langas pakeistas greitu įprastų šriftų pomeniu (Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman, Georgia), išlaikant dabartinį teksto dydį.
• Pagerinti RSS / tinklalaidžių pranešimai naudojant dvigubą būsenos modelį: šaltinio mazgai praneša „nauji elementai“, kai yra atnaujinimų, o atskiri RSS straipsniai ir tinklalaidžių epizodai – „neskaityta“ / „neklausyta“. Šį elgesį galima išjungti Parinktyse.
Klaidų taisymai
• Ištaisyta EPUB teksto ištrauka knygoms su įterptais HTML komentarais (<!-- ... -->): skyrių tekstas dabar analizuojamas teisingai ir nėra dalinai ar visiškai praleidžiamas.
• Ištaisytos ispaniško Wiktionary paieškos ir žodyno podėlio tvarkymas: tokie ispanų kalbos įrašai kaip „agua“ dabar įkeliami teisingai, o seni „Word not found“ podėlio įrašai nebenaudojami.
• Ištaisytas RSS straipsnių importo simbolių kodavimas kai kuriuose ispaniškuose šaltiniuose (pvz., El Mundo): kirčiuotos raidės ir „ñ“ dabar teisingai išsaugomos laikinajame redaktoriuje.
• Ištaisytas ANSI teksto dekodavimas Vidurio Europos failams (pvz., čekų / lenkų): Sonarpad dabar geriau atskiria UTF-8 ir ANSI bei pasirenka tinkamą kodų puslapį, įskaitant Windows-1250, kad nebūtų sugadinti diakritiniai ženklai.
• Ištaisytas RSS šaltinių su URL užklausos parametrais (pvz., `rss.aspx?c=...`) išsaugojimas: dabar jie teisingai išsaugomi ir atkuriami paleidus Sonarpad iš naujo.
• Ištaisytas Google Drive nuorodų failų (`.gdoc`, `.gsheet`, `.gslides`) atidarymas iš Explorer kontekstinio meniu: jei tiesioginis skaitymas nepavyksta su „Incorrect function (os error 1)“, Sonarpad dabar naudoja sistemos atidarymą, todėl dokumentas vis tiek atidaromas teisingai.
• Ištaisytas senų Excel 2010 `.xls` failų skaitymas: seni dvejetainiai Excel failai dabar aptinkami ir dekoduojami teisingai, o ne rodomi kaip sugadintas tekstas (pvz., `ÐÏ_à¡±...`).
• Ištaisytas rašybos tikrinimo pranešimų srautas: klaidingai parašyti žodžiai vėl pranešami vėliau peržiūrint tekstą, o ta pati klaida vėl pranešama, jei ji ištrinama ir įvedama iš naujo.
• Ištaisyti eilutėmis paremti teksto veiksmai (pvz., Ctrl+Q / Ctrl+Shift+Q, rikiuoti / apversti / palikti unikalius / sujungti eilutes): pasirinkus vieną eilutę su Shift+Žemyn gretimos eilutės nebėra sujungiamos ar nukerpamos.
• Ištaisytas kelių eilučių elgesys eilutėmis paremtuose veiksmuose (Ctrl+Q / Ctrl+Shift+Q ir susijusiuose įrankiuose): RichEdit pažymėjimai su tik CR skirtukais dabar normalizuojami teisingai, todėl visos pažymėtos eilutės apdorojamos nenukerpant pirmųjų simbolių.
• Išplėstas TTS įvesties normalizavimas matomiems tarpų simboliams (␠/U+2420, ␣/U+2423, ␉/U+2409, ␊/U+240A, ␍/U+240D, ␤/U+2424), kad daugiakalbiai balsai nekartotų pastraipų.
• Patobulintas Edge TTS teksto valymas naudojant vieną tikrinimo grandinę: keisti / nematomi tarpai normalizuojami, ilgos skyrybos sekos (pvz., „...“, „!!!“, „???“) sutrumpinamos, o tik iš skyrybos ženklų sudaryti fragmentai praleidžiami, kad nesusidarytų atkūrimo ciklai.
• Ištaisytas atkūrimo laiko pranešimas (Ctrl+I) MP3 / tinklalaidžių srautams: dabartinis laikas dabar ribojamas iki takelio trukmės, o atkūrimas automatiškai sustabdomas, jei pozicija viršija pabaigą.
• Pagerinta diegiklio lokalizacija: setup.exe dabar turi papildomas diegiklio kalbas (čekų, lenkų, prancūzų, serbų), o MSI paliktas kaip vienas en-US paketas, kad leidimuose nekiltų painiavos.
• Ištaisytas kontekstinio meniu įrašų valymas šalinant programą: „Atidaryti su Sonarpad“ dabar patikimai pašalinamas, įskaitant senesnius registro scenarijus.
• Ištaisytas SAPI5 pristabdymo / tęsimo patikimumas: F4 dabar teisingai pristabdo, o tęsimas grįžta į numatytą vietą, o ne pradeda nuo pradžių.
• Ištaisytas medijos pristabdymo + persukimo + tęsimo srautas: pristabdžius ir pakeitus vietą Kaire / Dešine, tarpo klavišas dabar patikimai tęsia nuo dabartinės vietos, o ne sustabdo ar pradeda iš naujo.

Versija 0.6.5 – 2026-02-07
Patobulinimai
• Pagerintas ispanų kalbos vertimas, ačiū Arturo Fernandez Rivas.
• Pridėta parinktis skaidyti EPUB garsines knygas pagal skyrius.
• RSS importai dabar naudoja atskirą laikiną skirtuką su lokalizuotu pavadinimu; „Išsaugoti kaip“ paverčia jį įprastu dokumentu.
• Ekrano skaitytuvo pranešimai dabar siunčiami ir JAWS, kai jis pasiekiamas.
Klaidų taisymai
• Skaitymas nuo žymeklio (F5) dabar prasideda tiksliai žymeklio vietoje. Anksčiau jis galėjo prasidėti keliomis eilutėmis aukščiau, nes žymeklio poslinkis neatitiko CRLF / UTF-16 pozicijų.
• Ištaisytas perpiešimo sutrikimas, kai rašant ant pažymėto teksto ankstesnis tekstas galėjo laikinai dingti iki pažymėjimo pakeitimo.
• Ištaisytas EPUB skyrių analizavimas, kad viršelio ar tik paveikslų puslapiai nebegeneruotų skaitomo CSS (pvz., „padding“) arba pavadinimų „Sconosciuto“.
• Ištaisytas EPUB garsinių knygų skaidymas pagal laiką su Edge TTS, kai tušti / per dideli fragmentai sukeldavo „Edge audio not sent“.
• RSS straipsniai dabar dekoduoja HTML esybes (pvz., &quot;, &amp;, &lt;, &gt;).
• Išsaugoti / Išsaugoti kaip dabar siūlo esamą failo vardą, kai išsaugomas neperrašomas formatas (pvz., EPUB), o ne pirmą teksto eilutę.
• Ištaisyta klaida, kai tinklalaidės su naujais epizodais nebuvo pranešamos kaip neklausytos; „Unheard“ pervadinta į „Unplayed“, kad pavadinimas būtų profesionalesnis.

Versija 0.6.4 – 2026-02-05
Patobulinimai
• Programa pervadinta į Sonarpad, siekiant pabrėžti garsą ir audio kaip pagrindinį akcentą.
• Atkūrimo meniu pridėtas garso takelio pasirinkimas medijos failams su keliais garso takeliais, pvz., MKV su keliomis kalbomis.
• Tinklalaidės dabar aiškiai pažymi neklausytus epizodus prefiksu „Unheard“ prieš pavadinimą.
• Naujas balsų perjungimas tekste naudojant žymas. Pavyzdžiai:
  - Microsoft balsai (Edge): <voice edge it-IT-IsabellaNeural>Hello</voice>
  - SAPI5 balsai: <voice sapi5 Microsoft Helena Desktop>Hello</voice>
  - SAPI4 balsai: <voice sapi4 #1>Hello</voice>
  - Su greičiu / tonu / garsumu: <voice edge it-IT-ElsaNeural speed=-20 pitch=-5 volume=-10>Hello</voice>
• Papildytos tinklalaidžių kategorijos.
• Pagerintas PDF skaitymas su automatiniu atsarginiu PDFium naudojimu.
• Pagerintas straipsnių analizatorius atvejams, kai turinys nebuvo perskaitomas visas.
• Atkūrimo meniu pridėtas tono atstatymas.
• Kontekstiniame meniu pridėta galimybė sukurti garsinę knygą iš pažymėto teksto.
• Pridėtas garsinių knygų skaidymas pagal trukmę su galimybe pasirinkti pirmojo failo vardą.
• Lokalizuota autoriaus žyma straipsnių skaityme (pvz., „by“, „di“, „par“).
• Pridėtos įtraukų parinktys (tabuliacija / tarpai ir plotis) bei Tab / Shift+Tab įtraukti / atitraukti pažymėtas eilutes.
• Ištaisytas Markdown valymas, kad `*` sąrašo žymės būtų tinkamai tvarkomos, kai sąrašo žymių išsaugojimas išjungtas.
• Pridėta parinktis naudoti seną pavadinimą „Novapad“ lango antraštėje ir Start meniu nuorodose.
Klaidų taisymai
• Ištaisyta klaida, dėl kurios SAPI4 garsinės knygos galėjo būti sukuriamos ne taip, kaip tikėtasi.
• Ištaisyta klaida, kai persukus už medijos failo pabaigos atkūrimas prasidėdavo nuo pradžių.
• Paieška failuose: Enter ant rezultato dabar atidaro tikslioje ištraukos vietoje, o Esc grąžina į rezultatus.
• Parinkčių langas: pagerintas vizualinis bendrųjų, balso, redaktoriaus ir garso skirtukų išdėstymas, kad valdikliai nebūtų praleisti ar nukirpti.
• Ištaisytas žymelių sutrikimas keičiant atkūrimo greitį.
• Ištaisytos neteisingai rodomos Podcast Index kategorijos.
• Ištaisytas skaitymo sutrikimas dėl apostrofų, pašalinus atskirą dialogų skaitymą; vietoje jo naudojamos balso žymos.

Versija 0.6.3 – 2026-01-30
Patobulinimai
• Pagerintas mikrofono aptikimas.
• Pridėtas momentinis atkūrimas visiems formatams.
Klaidų taisymai
• Ištaisyta tinklalaidžių kategorijų lango griūtis.

Versija 0.6.2 – 2026-01-30
Naujos funkcijos
• Pridėtas failų vykdymas (Shift+F5). Vartotojai Parinktyse gali pasirinkti interpretatorių (pvz., python), rasti jį kompiuteryje ir paspaudus Shift+F5 paleisti dabartinį scenarijų. HTML failai atidaromi naršyklėje.
• Pridėtas Google Docs nuorodų failų (.gdoc, .gsheet, .gslides) palaikymas; jie automatiškai atidaromi numatytojoje naršyklėje.
• Pridėtas M4B garsinių knygų formato (Apple/AAC) palaikymas.
• Tinklalaidžių paieškos rezultatų kontekstiniame meniu pridėta „Rodyti epizodus“, kad epizodus būtų galima naršyti ir leisti neprenumeruojant.
• Pridėta „Eiti į eilutę“ (Redagavimo meniu arba Ctrl+J), leidžianti greitai pereiti prie konkretaus eilutės numerio.
• Pridėtos kontekstinio meniu parinktys RSS kanalams ir tinklalaidėms rikiuoti abėcėlės tvarka arba pagal datą.
• Pridėti numatytieji vietnamiečių RSS kanalai.
• Įrašymo lange pridėtas mikrofono testas lygiui patikrinti prieš pradedant.
• Tinklalaidžių epizodų kontekstiniame meniu pridėta „Rodyti aprašą“.
• Per FFmpeg pridėtas išplėstinių garso / vaizdo formatų palaikymas: mkv, avi, mov, m4v, webm, mpg, ts, wmv, flv, vob, 3gp, flac, ogg, wma, aiff.
• Pridėtas sinchronizuotas subtitrų skaitymas (srt, vtt, ass, sub, sbv, lrc, smi) per NVDA arba pasirinktą balsą. Programa ieško subtitrų failo tuo pačiu vardu kaip medijos failas. Atkūrimo meniu pridėta „Importuoti subtitrus“ ir „Pašalinti subtitrus“, jei failų vardai skiriasi.
• „Atidaryti su Sonarpad“ kontekstiniame meniu pridėtos asociacijos visiems naujiems palaikomiems garso / vaizdo formatams.
• Pridėtas tono reguliavimo nustatymas bet kuriam failui.
• Bendruosiuose nustatymuose pridėta parinktis įjungti arba išjungti anoniminius klaidų pranešimus. Pagalbos meniu pridėta diagnostinio ZIP failo kūrimo komanda.
• Pridėta galimybė dialogams naudoti kitą balsą tiek tiesiogiai skaitant, tiek kuriant garsines knygas.
• Pridėta tinklalaidžių kategorijų naršyklė, skirta tyrinėti tinklalaides pagal kategoriją (verslas, menas, sportas ir kt.).
Patobulinimai
• Atidarius garso / vaizdo failą iš Explorer dabar iškart rodoma leistuvo peržiūra, o ne teksto redaktorius.
• Pašalinta OCR užklausa neprieinamiems PDF; OCR dabar atliekamas automatiškai, kad būtų greičiau ir patogiau.
• Pagerintas Prieinamas terminalas: NVDA skaitymas dabar prisimena paskutinę perskaitytą eilutę, kad skaitymas būtų nuoseklesnis.
• SAPI4: garsinių knygų kūrimas dabar visiškai lygiagretinamas ir beveik momentinis. Pridėta užklausa pasirinkti vienu metu vykdomų procesų skaičių.
• SAPI4: pašalintas WAV į MP3 butelio kaklelis, nes fragmentai konvertuojami lygiagrečiai sintezės metu.
• SAPI4: pagerintas klaidų tvarkymas ir automatinis laikinųjų failų valymas.
• Paieškos lange „Regex“ pervadinta į „Reguliarusis reiškinys“, kad būtų aiškiau, ir pridėti trūkstami paieškos parinkčių vertimai.
• M4B garsinės knygos: pagerintas išvesties tvarkymas; skaidant pagal dalis / žymeklius dabar gaunamas vienas M4B failas su tinkamais metaduomenų skyriais, įskaitant pavadinimą ir autorių.
• Leistuvas: pataisytas žymelių ir laiko pranešimo tikslumas, kai atkūrimo greitis nėra 1,0x.
• Parinktyse atkurtas naršymas Ctrl+Tab ir Ctrl+Shift+Tab.
• Atkūrimo meniu pridėta parinktis iškart atstatyti greitį į Normalų (1,0x).
• Visi priklausomumai atnaujinti į naujausias versijas, siekiant geresnio našumo ir stabilumo.
• FFmpeg integruotas su dinaminiu DLL įkėlimu, užtikrinant suderinamumą ir neblokuojant paleidimo.
• Tinklalaidžių atsisiuntimo filtrai atnaujinti įtraukiant naujus garso / vaizdo formatus.
• Ctrl+S nebeleidžia išsaugoti garso / vaizdo failų, kad jie nebūtų sugadinti.
• Pagerintas YouTube transkriptų importas, kad būtų patikimesnis ir atsparesnis.
• Pagerintas garsinių knygų skaidymas į dalis, užtikrinant, kad neprarandamas tekstas.
• Diegiklis dabar visiškai daugiakalbis: pagal sistemos kalbą palaikomos italų, anglų, ispanų, portugalų, švedų ir vietnamiečių kalbos. Nepalaikomoms sistemoms numatyta anglų kalba.
• Tinklalaidžių kategorijos: paspaudus Enter ant kategorijos pasirinkimas patvirtinamas, kaip paspaudus OK.
• Pagerinta pakibimų aptikimo sistema, kad nebūtų klaidingų pranešimų, kai atidaryti modaliniai langai (klaidų pranešimai, „tekstas nerastas“).
Taisymai
• Ištaisyta klaida, dėl kurios pakeitimų žurnalas neatsidarydavo paleidžiant.
• Ištaisyta klaida, dėl kurios OCR užklausa nepasirodydavo neprieinamiems PDF, atidarytiems iš Explorer.
• Ištaisyta paleidimo klaida, galėjusi iškart po atidarymo prarasti fokusą arba uždaryti langą.
• Ištaisyta kritinė reguliariųjų reiškinių paieškos klaida, neleidusi rasti teksto, įskaitant „Ieškoti nuo pradžios“ ir „Taškas atitinka naują eilutę“ problemas su Windows eilučių pabaigomis.
Lokalizacija
• Pridėtas lenkų kalbos vertimas.
• Pridėtas prancūzų kalbos vertimas.
• Pridėtas čekų kalbos vertimas, ačiū Radek Žalud ir Jiri Holzinger.

Versija 0.6.1 – 2026-01-20
Taisymai
• Ištaisyta klaida, kai įjungus „Rodyti balsus redaktoriuje“ sustodavo tinklalaidės atkūrimas.
• Ištaisyta problema, kai kai kurių tinklalaidžių nepavykdavo pridėti per URL, nes adresas būdavo nukerpamas.
• Ištaisyta klaida, dėl kurios į RSS kanalų funkciją nebebuvo galima pridėti įprastų URL.
• Ištaisyta problema, kai Wikipedia kalbos parinktis kelis kartus buvo rodoma skirtinguose nustatymų skirtukuose.
• Pašalintas derinimo failų kūrimas, kurie klaidingai buvo generuojami ir leidimo režimu.
Patobulinimai
• Pagerintas Microsoft balsų palaikymas; dabar naudojamas atskiras atkūrimo metodas su kitu user agent.
• Pridėtas MP4 failų palaikymas.

Versija 0.6.0 – 2026-01-20
Naujos funkcijos
• Pridėtas rašybos tikrintuvas. Kontekstiniame meniu galima patikrinti, ar dabartinis žodis teisingas, o jei ne – gauti rašybos pasiūlymų.
• Pridėtas tinklalaidžių importas ir eksportas per OPML failus.
• Be iTunes pridėtas Podcast Index paieškos palaikymas. Vartotojai gali įvesti nemokamą API raktą ir slaptąjį raktą, sukuriamus naudojant tik el. pašto adresą.
• Pridėtas SAPI4 balsų palaikymas tiek tiesioginiam skaitymui, tiek garsinių knygų kūrimui.
• Neprieinamiems PDF pridėtas automatinis OCR atsarginis būdas: jei nepavyksta išgauti teksto, dokumentas atpažįstamas OCR.
• Pridėtas Wiktionary žodynas. Paspaudus Programų klavišą rodomi apibrėžimai ir, jei yra, sinonimai bei vertimai į kitas kalbas.
• Pridėtas Wikipedia straipsnių importas su paieška, rezultatų pasirinkimu ir tiesioginiu importavimu į redaktorių.
• RSS modulyje pridėtas Shift+Enter, leidžiantis straipsnį atidaryti tiesiogiai originalioje svetainėje.
Patobulinimai
• Programa dabar visada paiso pasirinkto mikrofono.
• Tinklalaidės lange paspaudus Enter ant epizodo NVDA iškart praneša „įkeliama“, patvirtindama veiksmą.
• Tinklalaidžių paieškos rezultatuose Enter dabar prenumeruoja pasirinktą tinklalaidę.
• Pataisytos ir pagerintos Ctrl+Shift+O bei Tinklalaidė Ctrl+Shift+P sparčiųjų klavišų etiketės.
• Atkūrimo greitis ir garsumas dabar išsaugomi nustatymuose ir išlieka visuose garso failuose.
• Tinklalaidžių epizodams pridėtas atskiras podėlio aplankas. Epizodus galima išlaikyti per „Išlaikyti tinklalaidę“ Atkūrimo meniu. Podėlis automatiškai valomas viršijus vartotojo nustatytą dydį (Parinktys → Garsas).
• Žymiai pagerintas RSS straipsnių gavimas naudojant libcurl apsimetimą Chrome ir iPhone profiliais, užtikrinant suderinamumą su maždaug 99 % svetainių.
• RSS straipsniams pridėta skaityta / neskaityta būsena su aiškiu žymėjimu RSS sąraše.
• „Pakeisti viską“ dabar praneša atliktų pakeitimų skaičių.
• Naršant tinklalaidžių biblioteką su Tab pridėtas mygtukas Pašalinti tinklalaidę.
Taisymai
• Iš Pagalbos meniu pašalintas perteklinis „laukiantis atnaujinimas“ įrašas, nes atnaujinimai jau tvarkomi automatiškai.
• Ištaisyta klaida, kai Ctrl+S ant atidaryto MP3 failo jį išsaugodavo ir sugadindavo.
• Ištaisyta UI problema, kai „Paketinės garsinės knygos“ buvo rodoma kaip „(B)… Ctrl+Shift+B“; perteklinė žyma pašalinta.
• Ištaisytos išmaniosios kabutės: kai įjungtos, įprastos kabutės dabar teisingai pakeičiamos išmaniosiomis.
• Ištaisyta klaida, kai „Eiti į žymelę“ atstatydavo atkūrimo greitį į 1,0.
• Ištaisyta problema, kai jau atsisiųsti tinklalaidžių epizodai būdavo atsisiunčiami iš naujo vietoj podėlio versijos.
Spartieji klavišai
• F1 dabar atidaro pagalbos vadovą.
• F2 dabar tikrina atnaujinimus.
• F7 / F8 dabar pereina prie ankstesnės arba kitos rašybos klaidos.
• F9 / F10 dabar greitai perjungia mėgstamus balsus.
Kūrėjo patobulinimai
• Klaidos nebepametamos tyliai: pašalinti visi `let _ =` šablonai, o klaidos dabar aiškiai perduodamos, registruojamos arba tvarkomos tinkamu atsarginiu būdu.
• Projektas dabar nesikompiliuoja, jei yra įspėjimų: tiek cargo check, tiek cargo clippy turi baigtis švariai, su sugriežtintais lint tikrinimais ir, kur įmanoma, pašalintais `allow`.
• Pašalintos savos strlen / wcslen tipo pagalbinės realizacijos. Eilučių ir UTF-16 buferių ilgiai dabar gaunami iš Rust valdomų duomenų, o ne skenuojant atmintį.
• DLL tvarkymas sutvarkytas ir suvienodintas aplink libloading, atsisakant savos įkėlimo logikos ir PE analizės.
• Pašalinti savi baitų analizavimo pagalbininkai; visi baitai dabar analizuojami standartiniais from_le_bytes / from_be_bytes metodais ant patikrintų iškarpų.
Šie pakeitimai sumažina nereikalingą unsafe naudojimą, pašalina galimą neapibrėžtą elgesį ir daro kodo bazę idiomiškesnę, patikimesnę ir lengviau prižiūrimą.

Versija 0.5.9 - 2026-01-13
Naujos funkcijos
• Pridėtas RSS perrikiavimas iš kontekstinio meniu (aukštyn / žemyn / į poziciją) su neteisingų pozicijų tikrinimu.
• Straipsniams pridėtas kontekstinis meniu su originalios svetainės atidarymu ir bendrinimu per WhatsApp, Facebook ir X.
• Pridėtas Esc, leidžiantis grįžti iš importuotų straipsnių į RSS sąrašą.
• Pridėtas tinklalaidžių režimas: ieškoti, prenumeruoti, klausyti; perrikiuoti prenumeratas; Esc sustabdo atkūrimą ir grąžina į sąrašą; Enter ant epizodo pradeda atkūrimą.
• Tinklalaidėms ir MP3 failams pridėtas atkūrimo greičio valdymas.
• Pridėtas Ctrl+T, leidžiantis pereiti į konkretų laiką.
• Po garsumo sąrašo pridėtas balso peržiūros mygtukas.
• Pridėta paieška ir keitimas reguliariaisiais reiškiniais (Notepad++ stiliumi).
• Pridėtas RSS importas iš OPML ir TXT failų.
• Pridėta parinktis įjungti „Atidaryti su Sonarpad“ File Explorer, įskaitant nešiojamas versijas.
Patobulinimai
• Pagerintas balso greičio / tono / garsumo pasirinkimas, laikantis maksimalių TTS ribų.
• Įvairūs RSS patobulinimai, leidžiantys atsisiųsti visus straipsnius neperkeliant NVDA fokuso atnaujinimo metu.
• Pagerintas garso atkūrimas su atskiru meniu, Ctrl+I laiko pranešimu ir garsumu iki 300 %.
• Kai kurioms funkcijoms pridėti trūkstami spartieji klavišai.
• Redagavimo meniu pertvarkytas įtraukiant teksto valymo pomeniu.
• Parinktys pertvarkytos į skirtukus su Ctrl+Tab ir Ctrl+Shift+Tab navigacija.
• RSS skaitytuvas dabar atsisiunčia visą straipsnio turinį, atitinkantį naršyklės vaizdą.
Taisymai
• Ištaisytas Markdown valymas, pašalinantis skaičius eilučių pradžioje.
• Ištaisyta, kad AltGr+Z suaktyvindavo anuliavimą.
• Ištaisytas garsinės knygos įrašymo atšaukimas, kad sustotų greitai.
Lokalizacija
• Pridėtas vietnamiečių kalbos vertimas, ačiū Anh Đức Nguyễn.

Versija 0.5.8 - 2026-01-10
Naujos funkcijos
• Pridėtas mikrofono ir sistemos garso garsumo valdymas įrašant tinklalaides.
• Pridėta nauja funkcija importuoti straipsnius iš svetainių arba RSS kanalų, įskaitant svarbiausius kanalus kiekvienai kalbai.
• Pridėta funkcija pašalinti visas dabartinio failo žymeles.
• Pridėta funkcija pašalinti pasikartojančias eilutes ir iš eilės besikartojančias eilutes.
• Pridėta funkcija uždaryti visus skirtukus ar langus, išskyrus dabartinį.
• Pagalbos meniu visomis kalbomis pridėtas įrašas Aukos.
Patobulinimai
• Pagerintas prieinamas terminalas, siekiant išvengti kai kurių griūčių.
• Pagerinti ir pataisyti prieigos klavišai bei spartieji klavišai visoje programoje.
• Ištaisyta problema, kai uždarius garso atkūrimo langą atkūrimas nesustodavo.
• Svarbiems veiksmams pridėti patvirtinimo langai (pvz., pašalinti pasikartojančias eilutes, pašalinti eilučių pabaigos brūkšnelius, pašalinti visas dabartinio failo žymeles). Jei veiksmas netaikomas, dialogas nerodomas.
• Pridėta galimybė pašalinti RSS kanalus / svetaines iš bibliotekos, pažymėjus juos ir paspaudus Delete.
• RSS lange pridėtas kontekstinis meniu RSS kanalams / svetainėms redaguoti arba pašalinti.
• Pašalintas nustatymas perkelti konfigūraciją į dabartinį aplanką. Dabar programa tai tvarko automatiškai pagal vietą: jei exe aplankas vadinasi „sonarpad portable“ arba exe yra keičiamame diske, nustatymai saugomi exe aplanko `config`; kitu atveju `%APPDATA%\\Sonarpad`, o jei pageidaujamas aplankas neįrašomas – naudojamas exe `config`.

Versija 0.5.7 - 2026-01-05
Naujos funkcijos
• Pridėta paketinių garsinių knygų funkcija, skirta vienu metu konvertuoti kelis failus / aplankus.
• Pridėtas Markdown failų (.md) palaikymas.
• Atidarant teksto failus pridėtas koduotės pasirinkimas.
• Prieinamame terminale pridėta parinktis pranešti naujas eilutes per NVDA.
Patobulinimai
• Garsinės knygos įrašymas dabar išsaugo tiesiogiai MP3, kai pasirinktas šis formatas.
• Vartotojas dabar gali pasirinkti neišsaugotų pakeitimų žvaigždutės (*) vietą lango antraštėje.
• Pagerintas atnaujinimo sistemos patikimumas įvairiais scenarijais.
• Redagavimo meniu pridėta „Pašalinti brūkšnelius“, skirta OCR eilučių pabaigoms taisyti.

Versija 0.5.6 - 2026-01-04
Taisymai
  Pagerinta Paieška failuose, kad Enter atidarytų failą tiksliai pasirinktame ištraukos taške.
Patobulinimai
  Pridėtas PPT/PPTX palaikymas (atidaryti kaip tekstą).
  Atidarius ne tekstinius formatus dabar išsaugoma kaip .txt, kad nebūtų sugadintas formatavimas (PDF/DOC/DOCX/EPUB/HTML/PPT/PPTX).
  Pridėtas tinklalaidės įrašymas iš mikrofono ir sistemos garso (Failas meniu, Ctrl+Shift+R).

Versija 0.5.5 – 2026-01-03
Naujos funkcijos
• Pridėtas prieinamas terminalas, optimizuotas didelei išvesčiai ir ekrano skaitytuvams (Ctrl+Shift+P).
• Pridėtas nustatymas saugoti vartotojo nustatymus dabartiniame aplanke (nešiojamas režimas).
Taisymai
• Pagerintos Paieškos failuose ištraukos, kad peržiūra liktų sulygiuota su atitikmeniu.

Versija 0.5.4 – 2026-01-03
Patobulinimai
• Ištaisyta „Normalizuoti tarpus“ (Ctrl+Shift+Enter).
• Pridėtas HTML/HTM palaikymas (atidaryti kaip tekstą).

Versija 0.5.3 – 2026-01-02
Naujos funkcijos
• Pridėta Paieška failuose.
• Pridėti nauji teksto įrankiai: Normalizuoti tarpus, kietasis eilutės lūžis ir pašalinti Markdown.
• Pridėta Teksto statistika (Alt+Y).
• Redagavimo meniu pridėtos naujos sąrašo komandos:
• Rikiuoti elementus (Alt+Shift+O)
• Palikti unikalius elementus (Alt+Shift+K)
• Apversti elementus (Alt+Shift+Z)
• Pridėta Cituoti / panaikinti eilučių citavimą (Ctrl+Q / Ctrl+Shift+Q).
Lokalizacija
• Pridėta ispanų lokalizacija.
• Pridėta portugalų lokalizacija.
Patobulinimai
• Kai atidarytas EPUB failas, Išsaugoti dabar automatiškai persijungia į Išsaugoti kaip ir eksportuoja turinį kaip .txt, kad EPUB nebūtų sugadintas.

## 0.5.2 - 2026-01-01
- Pridėtas pakeitimų žurnalas.
- Diegimo metu pridėtos „Atidaryti su Sonarpad“ parinktys ir palaikomų failų susiejimai.
- Pagerinta pranešimų lokalizacija (klaidos, dialogai, garsinių knygų eksportas).
- Pridėtas dalių pasirinkimas naudojant „Skaidyti garsinę knygą pagal tekstą“, su parinktimi „Reikalauti žymeklio eilutės pradžioje“.
- Pridėtas YouTube transkriptų importas su kalbos pasirinkimu, laiko žymų parinktimi ir pagerintu fokuso valdymu.

## 0.5.1 - 2025-12-31
- Automatiniai atnaujinimai su patvirtinimu, pagerintas klaidų tvarkymas ir pranešimai.
- Garsinių knygų eksporto patobulinimai (skaidymas pagal tekstą, SAPI5/Media Foundation, išplėstiniai valdikliai).
- TTS patobulinimai (pristabdyti / tęsti, pakeitimų žodynas, mėgstami balsai).
- Rodymo meniu ir balsų / mėgstamų balsų skydeliai, teksto spalva ir dydis.
- Numatytoji kalba pagal sistemos lokalę ir lokalizacijos patobulinimai.
- CI ir Windows paketavimas (artefaktai, MSI/NSIS, podėlis).

## 0.5.0 - 2025-12-27
- Modulinis pertvarkymas (redaktorius, failų tvarkymas, meniu, paieška).
- Windows kūrimo / paketavimo workflow ir README / licencijos atnaujinimai.
- Ištaisytas TAB naršymas Pagalbos lange.

## 0.5 - 2025-12-27
- Preliminarus versijos padidinimas.

## 0.1.0 - 2025-12-25
- Pirmasis leidimas: projekto struktūra ir README.
