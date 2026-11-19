# Değişiklik günlüğü

Sürüm 0.9.32 – 2026-11-19

1. Anahtar kelimelerle oluşturulan RSS kaynakları artık daha güvenilir biçimde yükleniyor. Google Haberler isteği 4 saniyeden uzun sürerse Sonarpad makaleleri almak için otomatik olarak başka bir yöntem dener.
2. SonarTube ve SonarMusic’te bir videoda birden fazla ses parçası varsa oynatma sırasında oynatma menüsünden ses parçasını değiştirebilirsiniz.

Sürüm 0.9.31 – 2026-11-19

1. Gemini sesli betimleme oluşturulurken bir bölümü engellerse Sonarpad bu bölümü birer dakikalık parçalar hâlinde yeniden işler. Bir parça hâlâ engelleniyorsa o parçanın özgün sesini korur ve videonun geri kalanıyla devam eder.

2. RSS makalelerinde Türkçe seçildiğinde ana haberler, kategoriler ve yeni anahtar kelime aramaları için artık Google Haberler’in Türkiye sürümü kullanılıyor. Bir kaynak açılırken yinelenen istek kaldırıldı.

Sürüm 0.9.30 – 2026-11-19

1. Favoriler: SonarTube, SonarMusic ve radyo bağlam menüsüne “Alfabetik sırala” eklendi; podcast aboneliklerinde de doğrudan kullanılabilir. Sıralama kaydedilir ve ekran okuyucu tamamlandığını duyurur.

2. TMDB bir filmi veya diziyi animasyon olarak tanımladığında Sonarpad, yapımı otomatik olarak AniList’te arar. Karakter çizimleri Gemini’nin sahnede kimin göründüğünü tanımasına yardımcı olur. Güvenilir bir eşleşme bulunamazsa veya görsel yoksa sesli betimleme her zamanki gibi devam eder.

3. Sonarpad Windows için eksiksiz Türkçe çeviri: arayüz, ayarlar, 365 özel gün ve 128 alıntı içeren takvim, sesli betimlemeler, dil seçimleri, yardım, bağış bilgileri ve değişiklik günlüğü. Türkçe artık uygulamanın dilleri arasında yer alıyor.

4. Sonarpad artık container.xml dosyasının arşiv içindeki yolunda büyük ve küçük harf kullanımı farklı olan EPUB kitaplarını da açıyor. Daha önce bu kitaplar açılırken dosyanın arşivde bulunamadığını belirten bir hata gösteriliyordu.

Sürüm 0.9.29 – 2026-11-16

1. TMDB'den oyuncular ve fotoğraflar içeren sesli açıklamalar: filmin veya dizinin başlığını ve isteğe bağlı olarak yılı girin; prodüksiyonu, sezonu ve bölümü seçin. Gemini, oyuncuların isimlerini ve fotoğraflarını, tanınmaya yardımcı olmak amacıyla, bunları belirli bir kimlik olarak değerlendirmeden alır. Kataloğu saklayarak, daha sonraki kullanımlar için kurtarılacak olan fotoğrafların bağlantılarını kaydedersiniz. Animasyonlu içerik için sesli betimleme bu hizmeti kullanamaz çünkü seslendirme sanatçılarının fotoğraflarını sağlar ve bu da çizgi film karakterlerinin tanınmasına yardımcı olmaz. Bazen TMDB oyuncu kadrosundaki tüm karakterlerin fotoğraflarını sağlamadığından, bu sistemin uygulanması karakterlerin %100 tanınacağını garanti etmez. Ancak tanınma güvenilirliğini büyük ölçüde artırır.

2. **Yeni: Sonarpad'deki Gemini sesleri!** Sesli kitaplar ve sesli açıklamalar oluşturmak, haberleri, kitapları ve belgeleri okumak ve program işlevlerinde metinden konuşmaya kullanmak için insan sesine yakın, doğal, etkileyici sesler. Sonarpad AI'yi kendi erişim kodunuzla veya kendi Gemini API anahtarınızla seçebilirsiniz.

Sürüm 0.9.28 – 2026-11-13

1. Editör: Ctrl+X ve Düzenle > Kes, seçilen metin gerçekten kaldırıldığında artık NVDA veya JAWS ile "Kes"i duyurmanız yeterli.

2. Radyo Arama: Alternatif Rock, Film Müzikleri ve Video Oyun Müziği Eklendi. Hatırlanacak özgür bir tür yazmak da mümkündür.

3. Metin çevirisi: Bozuk DeepL entegrasyonu kaldırıldı. Çeviri artık iPhone'da olduğu gibi yalnızca Google Çeviri'yi kullanıyor.

4. Bulut Senkronizasyonu: Şarkılar, albümler, sanatçılar ve çalma listeleri dahil olmak üzere Windows, Mac ve mobil cihazlar arasında SonarMusic favorileri eklendi. Mevcut listeler birleştirilir ve açık kaldırma işlemleri senkronize edilir.

5. Podcast'ler: Windows önbellekten bir bölümü oynattığında bile Windows, Mac ve mobil arasında otomatik yer imi ve dinleme noktası senkronizasyonu eklendi. Eski konumlar yeni konumların üzerine yazılmaz; Bir yer imini tamamlama ve kaldırma işlemleri de senkronize edilir.

Sürüm 0.9.27 – 2026-11-11

1. SonarTube ve SonarMusic: Uzun bir duraklamadan sonra oynatmanın donması düzeltildi. Ses ve video kaydedilen konumdan devam ettirilir.

2. Yerleri ve etkinlikleri arayın: İtalyanca olarak kalan "Yeni arama" ve "Tümünü aç" düğmeleri tüm dillere çevrildi.

3. YouTube: Bir video erişim gerektirdiğinde, genel "yt-dlp başarısız oldu" mesajı yerine, gereksiz girişimlerden kaçınarak mesajı temizleyin.

4. Konuşma sözlüğü: diğer pencerelerden güncelleme ve kaydetme sırasında kayıplara karşı koruma; Dosyanın hasar görmesi durumunda kurtarma ile ayarların yedek bir kopyası eklendi.

5. Rotalar: seçilen rotada Esc tuşuna basıldığında alternatiflere dönülür ve arama tekrarlanmadan önceki seçim korunur.

Sürüm 0.9.26 – 2026-11-07

1. Yerleri ve işletmeleri arama artık tüm dillerde, uluslararası sonuçlar, açıklamalar, derecelendirmeler, inceleme sayısı, çalışma saatleri ve mümkün olduğunda web sitesiyle birlikte kullanılabiliyor. İtalyanca arayüzde Beyaz ve Sarı Sayfaların yerini alır ve aynı zamanda sonuçları, isim aramayı ve telefon numarası aramasını da korur. Diğer dillerde yalnızca yer ve etkinlik araması mevcuttur.

2. Daha kesin sonuçlar ve toplu taşıma rotalarının eklenmesiyle iyileştirilmiş rota arama.

3. AI Sesli Açıklamalar: "Diyalog ve yorum sırasında sesli açıklamalara izin ver" seçeneği eklendi, varsayılan olarak devre dışı bırakıldı. Sürekli yorumların Sonarpad açıklamalarına çok az yer bıraktığı veya hiç yer bırakmadığı futbol oyunları, güreş maçları ve diğer spor etkinliklerinin yanı sıra müzik videoları için de özellikle kullanışlıdır. Etkinleştirilerek, konuşma veya şarkı söyleme sırasında açıklamalar birbiriyle örtüşmeden de eklenebilir. Seçim, daha sonraki değişiklikler ve yenilemeler için de projede korunur. Devre dışı bırakılırsa normal diyalog koruması değişmeden kalır.

Sürüm 0.9.25 – 2026-11-06

1. Windows, Mac ve mobil arasında bulut senkronizasyonu eklendi: radyo ve TV favorileri, kişisel kaynaklar ve haber klasörleri, podcast'ler ve SonarTube favorileri, geçici kod bağlantısıyla. Favori öğeler desteklendiği yerde saklanır. Silme yönetimi ve teşhis günlükleriyle manuel senkronizasyon. Arama türü, hava durumu şehri ve kesim türü de dahil olmak üzere uyumlu modül tercihleri ​​de koleksiyonlar ve tercihler için ayrı kontroller ve tercihler için 32 KB sınırıyla senkronize edilir. Basitleştirilmiş sihirbaz: ilk oturum açtığınızda kodu başlatmayı veya girmeyi seçersiniz; Senkronizasyon eylemleri bağlı cihazlarda görünür. Seçenekler ayrı. Artık yerel olarak kalan ve RSS kaynağı olarak değerlendirilmeyen dahili Favori Makaleler girişinin neden olduğu senkronizasyon hatası düzeltildi. Değişikliklerden sonra toplu güncellemelerle, açıldığında ve kullanım sırasında arka planda otomatik senkronizasyon. Bulut penceresi durumu ve son senkronizasyonu gösterir. Sunucu yanıt vermezse uygulama kullanılabilir durumda kalır: otomatik uyarı yoktur, günlükte hatalar vardır ve artan beklemelerle yeni denemeler yapılır. Tercihlerin senkronizasyonu, Sonarpad aktivasyonunu, kodu yeniden girmeden diğer bağlı cihazlara da aktarır.

2. SonarTube, akıştan ses oynatmanın yeni adıdır ve halihazırda bilinen özelliklerin aynısını korur. YouTube aramalarında Tümü, Videolar, Oynatma Listeleri, Kanallar, Kurslar veya Podcast'leri seçebilirsiniz; seçim hatırlanır ve doğrudan adreslerin açılmasını değiştirmez.

3. Akış indirmesi: Ağ zaman aşımı ve yeniden bağlanma etkinleştirilerek tamamlanmadan önce oluşan çökme düzeltildi. Okuma hataları artık raporlanıyor ve tamamlanmamış bir dosyanın tamamlanmış sayılması önleniyor.

4. SonarTube ve SonarMusic: Sabit Yer imine git. Videolar ve şarkılar, normal oynatmayla aynı kalite seçimi ve otomatik geri dönüş stratejileriyle yeniden açılır ve kayıtlı konumdan devam edilir. Ayrıca orijinal bağlantıyı kurtarmanın mümkün olduğu durumlarda eski yer imleriyle uyumluluk da geliştirildi.

5. Normal ve gelişmiş kesme: Düzenlenen dosyanın tamamını veya tek bir bölümünü, diğerlerini silmeye gerek kalmadan kaydetmek için bir kombinasyon eklendi.

6. Beyaz Sayfalar bölümüne numaraya göre arama eklendi.

7. AI Ses Açıklamaları: Gemini videoyu reddettiğinde ve eski AVI'lerdeki DIV3 gibi uyumsuz bir codec bileşeni için MP4 hazırlığı da başarısız olduğunda son bir H.264/AAC yeniden kodlama geri dönüşü eklendi. Mevcut girişimler öncelik olmaya devam ediyor; orijinal dosya değiştirilmez.

Sürüm 0.9.24 – 2026-10-31

1. YouTube'dan yayın oynatma: Doğrudan akış, oynatılabilir ses verisi olmadan sona erdiğinde videoların sessiz açılması düzeltildi. Sonarpad artık bu hatayı algılıyor ve yt-dlp/HLS aracılığıyla otomatik olarak yeniden dener.
2. Parola korumalı PDF'ler: Sonarpad artık erişilebilir bir alanla parola istiyor ve doğruladıktan sonra belgeyi açıyor.
3. Dosya menüsüne "PDF'yi şifresiz kaydet" öğesi eklendi: belgeyi doğru şifreyle açtıktan sonra, bir kopyasını şifresiz kaydetmenize, düzeni ve içeriği korumanıza ve orijinali değiştirmeden bırakmanıza olanak tanır. Bir ekran okuyucu uyarısı, açıldıktan sonra bu olasılığı belirtir.

Sürüm 0.9.23 – 2026-10-30

1. YouTube'dan sesli açıklamalar: Ses ve video ayrı ayrı indirildiğinde ancak birleştirilmediğinde neredeyse sonunda ortaya çıkan akışlı indirme hatası düzeltildi. Sonarpad artık tüm parçaları yeniden kodlamaya gerek kalmadan yerleşik kitaplıklarla birleştiriyor.

2. YouTube'dan Sesli Açıklamalar: İndirilen dosyalar artık video başlığını kullanıyor. Oynatma bağlamında eksikse Sonarpad kaydetmeden önce onu alır; video kimliği yalnızca başlık mevcut olmadığında kullanılır.

3. Öğelerin seçimi: listelerin güncellenmesi, artık seçilen öğenin yerine kaydedilen öğeyi geri yüklemez. Ana ses ve iki diyalog sesi kendi seçimlerini korur; Açıkça bir profil seçildiğinde ayarları uygulanır.

4. AI Sesli Açıklamalar: Kesintiye uğrayan çalışmaya devam edilirken bile ilerleme penceresine başvurmak daha kolaydır. Yüzde, segment ve durum, oklarla okunabilen bir metin alanında ayrı satırlarda yer alır. Sekme, alan ve İptal arasında geçiş yapar; yerel ilerleme çubuğu NVDA geri bildirimi için kullanılabilir durumda kalır.

5. AI Sesli Açıklamalar: İptal edin ve çalışırken pencereyi kapatın, şimdi Evet/Hayır onayını isteyin. Çalışma ancak kullanıcının açık onayı sonrasında durdurulur.

Sürüm 0.9.22 – 2026-10-29

1. Sesli açıklamalar: Başlangıçtaki süre tahmininden hariç tutulan cümlelerin son kurtarması eklendi. Sonarpad, sesin gerçek süresini kontrol eder ve gerektiğinde, önceden eklenmiş olan açıklamaları hareket ettirmeden veya diyalogların üst üste binmesine yol açmadan %10'un üzerinde veya en fazla %15'lik bir hızla tekrar dener. Alınan açıklamalar ve varsa spesifik hızları projeye kaydedilir.

2. Google Voices: Chrome'un bir dosyayı geçici olarak meşgul etmesi sonucu oluşan başlatma hatası düzeltildi. Sonarpad otomatik olarak tekrar deneyecek ve donma devam ederse aynı Google girişini koruyarak Microsoft Edge'i deneyin. Normal çalışma değişmeden kalır.

3. YouTube oynatma listelerini indirin: Bir öğe HTTP 503 gibi geçici sunucu hataları nedeniyle başarısız olduğunda alternatif yt-dlp profiliyle otomatik bir deneme eklendi. Ayrıca erişilebilir ilerleme alanı da düzeltildi: Sekme ile çıkıp yeniden girmek zorunda kalmadan yüzde güncellemeleri.

Sürüm 0.9.21 – 2026-10-27

1. Akıştan ve SonarMusic'ten ses çalın: YouTube videoları yüklenirken bekleme penceresi ve yerelleştirilmiş hata mesajlarıyla birlikte arayüz çökmeleri düzeltildi. Sunucu HTTP 500'ü döndürürse, normal oynatma yolunu değiştirmeden otomatik olarak alternatif bir HLS kaynağı denenir; hata yalnızca kurtarma işleminin de başarısız olması durumunda gösterilir. Geliştirilmiş yükleme hatası teşhisi.
2. SonarMusic: Başka bir dil seçildiğinde bile bazı eylemleri İtalyanca olarak gösteren bağlamsal menü düzeltildi. Sesli açıklamaların kaydedilmesi, yazıya dönüştürülmesi ve oluşturulması artık ayarlanan dile uygundur.

3. Yer imlerinin genel yönetimi: Arama, tarihe veya dosyaya göre sıralama, yaşa ve var olmayan dosyalara göre filtreleme, kayıtlı konuma açma, yedek kopyalar olmadan birden fazla silme işlemini onaylama ile tüm yer imlerine bakmak için Ekle menüsündeki yeni pencere. Eski Bookmarks.json ile uyumlu ve NVDA ve JAWS tarafından erişilebilen, son açılan ve son güncellenen verilerin ayrı takibi.
4. YouTube: Uzun bir aradan sonra Sonarpad, videoya yeni bir bağlantı ister ve önceki konumdan devam etmeye çalışır; ilerleme kontrolü engellenen akışları kurtarmaya çalışır. Arama sonuçları sayfaları, okuyucuya döndüğünüzde sıralamayı, konumu ve odağı korumak için hatırlanır. Sonuçlar, aramanın döndürdüğü orijinal sıraya uygundur: Bu nedenle kanallar, videolardan ve oynatma listelerinden sonra taşınmadan ilk önce de görünebilir. HTTP 403/500 aktarım sorunlarının ve geçici olarak kullanılamayan formatların genişletilmiş kurtarması; özel içeriğin veya DRM'nin atlanması yok.
5. Video indirme: yt-dlp'nin WebM, MP4 veya belirli bir parçayı empoze etmeden formatı seçmesine olanak tanıyan Otomatik (orijinal) seçeneği eklendi; WebM, gerçek WebM dosyalarına ayrılmış bir seçenek olmaya devam ediyor. Kaydedilen dosya gerçek kapsayıcı uzantısını korur. SonarMusic: Şarkı, albüm, sanatçı ve çalma listesi bağlantılarını Ctrl+C tuşlarına basarak veya bağlam menüsünden favorilere bile kopyalayın.
6. Podcast Kaydı: "Durdur ve Kaydet"in yanına "İptal Et ve Sil" butonu eklendi. Onaylandıktan sonra, duraklatılmış olsa bile mevcut kaydı durdurur ve herhangi bir ses dosyasını kaydetmeden geçici dosyaları siler. “Durdur ve Kaydet” butonu eskisi gibi çalışmaya devam ediyor.
7. Ses dışa aktarma ve ses açıklamaları: ses frekansı değiştiğinde MP3 oluşturmada oluşan hata düzeltildi. Diğer formatlar için de yeniden örnekleme ve son numunelerin iyileştirilmiş yönetimi ve kodlayıcılar tarafından desteklenen frekanslarla uyumluluk.

Sürüm 0.9.20 – 2026-10-25

1. Hava Durumu: Arama sonuçlarında ve güncel şehirlerde iyileştirilmiş coğrafi işaretler. Mevcut olduğunda ilçe veya ilçe, bölge ve ülke de gösterilir, böylece aynı adı taşıyan yerleri ayırt edebilirsiniz. Tekrarlanan adlar ortadan kaldırıldı ve önceden kaydedilmiş şehirlerle uyumluluk korundu.
2. Klavye komutları, ekran okuyucuda gezinme, gelişmiş kırpma, arama ve sık kullanılanlar dahil olmak üzere Medya Dosyalarını Kırp ve SonarMusic için ayrıntılı talimatlar içeren güncellenmiş kılavuzlar.
3. Medya Dosyalarını Kes: Yeniden sıkıştırma olmadan kesmelere devam etmek ve dışa aktarılan dosyaları değiştirmeden önce yedeklemeler yapmak için köşeli parantez ve Sil içeren isteğe bağlı GoldWave tarzı anahtarlar eklendi.
4. Medya dosyasını kes: Onay ile orijinal dosyanın üzerine yazmak, geçici dosyaya güvenli aktarım ve otomatik yedekleme kopyası için kutu eklendi; kesme sırası kaydedildikten sonra güncellenir.
5. Medya dosyasını kes: Kılavuzlu ve gelişmiş modlarda, imleci hareket ettirmeden ve zaten uygulanmış olan kesmeleri iptal etmeden yanlış bir seçimi yeniden başlatmak için İşaretçileri sıfırla (Alt+0) eklendi.
6. Medya Dosyasını Kes: GoldWave Stili Komutlarını Kullan ve Orijinal Dosyanın Üzerine Yaz tercihleri, yeniden başlatma sonrasında bile hatırlanır; Üzerine yazma, onay ve yedek kopyayı korur.
7. YouTube ve SonarMusic: Kanalları, oynatma listelerini, albümleri ve sanatçıları tanımak için iyileştirilmiş sonuç etiketleri. Meta verilerde tekrarlardan kaçınarak, kullanılabilir olduğunda görünümleri geri yükleyin ve süre saat, dakika ve saniye cinsinden ifade edilir.
8. Akıştan ses oynat: Yeni YouTube sonuçlarındaki kanal abonelerinin sayısı düzeltilerek tanıtıcıdan ayırt edildi. Ayrıca, mümkün olduğunda SonarMusic sanatçılarında abonelerin görüntülendiği de doğrulandı.
9. Medyayı Kaydet: yt-dlp yoluyla indirmeler için mevcut en iyi kaliteye geri yüklenen WEBM formatı. Ayrı ses ve video, harici bir ffmpeg.exe olmadan, dahili FFmpeg kitaplıklarıyla yeniden sıkıştırmaya gerek kalmadan birleştirilir.

Sürüm 0.9.19 – 2026-10-24

1. Sesli açıklamalar: Anlatım için yer olmayan uzun sürelerden sonra yapılan bölüm, artık açıklamalarla aynı minimum duraklamayı kullanıyor ve kısa duraklamaları da tanıyor.

Sürüm 0.9.18 – 2026-10-24

1. Araçlar → Multimedya'ya Medya Dosyasını Kırp eklendi: ses ve videonun rehberli ve gelişmiş kırpılması, silinecek parçaların önizlemesi, sınırların ince ayarı ve silinen kısımların geri yüklenmesi. Oynat, halihazırda uygulanmış kesmeleri atlıyor. Video döndürme ve ses seviyesi ayarlanabilir ve tekrarlı bir ses parçası ekleme seçenekleri mevcuttur. Dahili FFmpeg kitaplıkları aracılığıyla gerçek ileri kaydetme ve geri alma; harici yürütülebilir dosya yok; erişilebilir sesli talimatlar ve komutlar.
2. Klavye kesme komutlarını Alt veya Ctrl olmadan kullanmak için Medya Dosyasını Kes'e bir komut alanı eklendi. F1 mevcut tuşların listesini okur.
3. Mac sürümü etiketleriyle Klasörü Araçlar → Multimedya'ya Dönüştür (Alt+Shift+Q) eklendi: ses ve videonun toplu dönüştürülmesi, sesten video oluşturmak için kalite ve görüntü seçimi. Gerçek ilerlemeyi, durmayı, hata özetini ve mevcut dosyaların korunmasını içerir. Yalnızca dahil edilen FFmpeg kitaplıklarını kullanın.
4. Araçlar → Multimedya'ya Medya Dosyalarını Birleştirme eklendi (Alt+Shift+Y): dosyaları ekleme, kaldırma ve yeniden sıralama düğmeleri, hedef format seçimi, gerçek ilerleme ve iptal düğmeleriyle birden fazla ses veya video dosyasını seçtiğiniz sırayla birleştirin. Videolar ayrıca tek bir ses dosyasında birleştirilebilir.
5. Medya dosyasını kes: önceden yapılmış olan kesmeleri koruyarak ses veya videoyu yeni bir parça olarak eklemek için “Medyayı seçilen parçadan önce ekle…” eklendi. Oynatma ve dışa aktarma yeni düzeni takip eder; eklenen kısım bölünebilir, silinebilir ve geri yüklenebilir. Videolarda yalnızca ses içeren kısımların arka planı siyahtır.
6. PAPATYA kitaplarının iyileştirilmiş oynatımı: sonraki bölüm otomatik olarak başlar ve oklarla ileri ve geri hareket ederek bölümler arasındaki sınırları geçer. Alt bölümler tekrarlanmıyor ve okuma sonu duyurusu sadece kitabın sonunda yapılıyor.
7. Yeni multimedya araçları için güncellenmiş arayüz, yardım ve çevirilerle Endonezce (Bahasa Endonezya) eklendi. Çeviriye katkılarından dolayı Muhammed'e teşekkür ederiz.

8. Multimedyaya SonarMusic eklendi: şarkıları, albümleri, sanatçıları, çalma listelerini ve müzik videolarını arayın; öneriler yalnızca yazıldıktan, sonuçlarda gezinildikten ve sayfalara ayrıldıktan sonra görünür. Mevcut medya araçlarını kullanan şarkılar için AI oynatma, kaydetme, transkripsiyon ve sesli açıklama mevcuttur.
9. YouTube videolarını aramayı ve açmayı hızlandırın.
10. Geliştirilmiş sesli açıklamalar: Ekrandaki metinlerin okunması etkinleştirilerek Gemini'nin tanımlayabildiği jenerikler de okunur.
11. Sesli açıklama senkronizasyonu iyileştirildi: Anlatım için yer olmayan uzun sürelerden sonra Gemini'nin mevcut sahneyi takip etmek yerine önceki sahneleri anlatmaya devam edebilmesi sorunu düzeltildi.
12. Sesli açıklamalar: Bir cümle mevcut alana sığmazsa, Sonarpad onu sentez hızı %10 veya gerekiyorsa %15 artırılarak yeniden oluşturmaya çalışır ve gerçek süreyi doğrular. Hız yalnızca söz konusu açıklama için kaydedilir ve sonraki düzenlemelerde, önizlemelerde ve dışa aktarmalarda korunur; diğer cümleler değişmeden kalır.

13. Sesli açıklamalar: Gerekli minimum duraklama 1,5 saniyeye düşürüldü. Sonarpad böylece daha önce kullanılmamış kısa duraklamalarda bile daha fazla açıklama üretebilir.
14. Aynı klasördeki dosyaları oynatma: Önceki Şarkı ve Sonraki Şarkı, ayrıca Ctrl+PageUp ve Ctrl+PageDown ile artık ses ve video dosyalarında ad sırasına göre ilerleyin. Seçenekler → Ses'te, geçerli dosya bittikten sonra bir sonraki multimedya dosyasını otomatik olarak oynatmak için varsayılan olarak devre dışı bırakılmış yeni bir onay kutusu vardır. Seçim hatırlanıyor; manuel komutlar, onay kutusu devre dışı bırakıldığında bile çalışır.

Sürüm 0.9.17 – 2026-10-15

1. Microsoft Visual C++ Yeniden Dağıtılabilir x64 için bir başlangıç kontrolü eklendi: eksik veya kullanılamıyorsa, erişilebilir bir mesaj resmi Microsoft bağlantısını sunar. Sonarpad'in başlatılması artık bu çalışma zamanının DLL'lerine bağlı değildir, dolayısıyla mesaj, bileşen eksik olduğunda bile gösterilebilir.
2. İleri veya geri sarma sırasında oynatma sırasında olası bir donma düzeltildi: Sonarpad artık mpv'den aynı anda alınan birden fazla mesajı doğru bir şekilde işliyor ve iletişim kanalı kapalıysa beklemeyi durduruyor.
3. Devam sırasında bile sesli açıklama ses ayarlarında dillerin ve seslerin yüklenmesi düzeltildi: Microsoft'a geçiş, eksik sesleri yükler ve diğer motorlardan gelen sonuçlar mevcut seçimin üzerine yazılmaz. Bir hata durumunda, Girişleri yeniden yükle düğmesiyle birlikte bir mesaj gösterilir.
4. İsteğe bağlı ad, sorunun açıklaması ve ZIP ekiyle birlikte yazara e-posta yoluyla teşhis gönderimi eklendi. Manuel kaydetme ve hata durumunda yeniden deneme olanağı mevcuttur; kimlik bilgileri tanılamadan kaldırılır. İletişime geçilecek isteğe bağlı bir e-posta belirtmek mümkündür: geliştirici, rapora doğrudan yanıt verebilir.

Sürüm 0.9.16 – 2026-10-14

1. AI ile sesli açıklama oluştur penceresi düzeltildi: Çalışma sırasında yüzde, filmin bit hızı ve ses seviyesiyle karıştırılmadan sesli açıklamanın ilerlemesini açıkça gösterir. 17 dilin tamamındaki etiketler güncellendi.

2. Ses değişikliklerini Medyayı Dönüştür'e dönüştürün ve Ctrl+Shift+A tuşlarını basılı tutarak Konuşma'dan Araçlar → Multimedya'ya geçin. Ffmpeg.exe olmadan yalnızca dahil edilen FFmpeg kitaplıklarını kullanarak ses dönüştürmeyi, videodan ses çıkarmayı, MP4/AVI/MOV'u, resimli sesten video ve 16/24/32 bit veya 32 bit kayan WAV'ı destekler.

3. AVI parçalarının OGG dönüşümü ve zamanlaması ve serpiştirilmesi düzeltildi. Sonuçlar öncelikle geçici dosyalara kaydedilir: hatalar veya iptaller mevcut bir hedefin yerini almaz. Yaklaşık 30 saniyelik gerçek klipler üzerinde 60 test dönüşümü gerçekleştirin.

4. AI Ses Tanımı Oluştur özelliği, videonun görüntülenmesine gerek kalmadan Akıştan Sesi Çal yoluyla oynatıldığında bile uyumlu video akışları için kullanılabilir hale getirildi.

5. Sesli açıklama indirme dosyasındaki format seçimi düzeltildi ve aynı zamanda eksik codec bilgilerine sahip akışlar da kabul edildi. Medyayı Kaydet'ten çok daha yavaş bir akışı seçebilen 720p tercihi kaldırıldı. Günlüklere indirme hızı, boyutu, biçimi, süresi ve ilerleme durumu eklendi.

6. Uyumluluk modunda hazırlanan eski kayıtlar ve bölümler olsa bile, kesintiye uğrayan sesli açıklamaların iyileştirilmiş şekilde sürdürülmesi. Biçim değiştirme, zaman sınırları eşleştiğinde tamamlanan bölümleri korur; uyumsuz durumlarda kaydetmeyi durdurur ve korur. Kaydetme işlemini daha güvenli hale getirdik ve sahne zamanı referanslarını koruduk.

7. Sesli açıklama köprüsünün önbelleği v10'dan v11'e güncellendi: güncellemeden sonraki ilk kullanımda, bileşenin yeni kopyası indirilir ve bu kopyanın yeniden derlenmesi ve sürümle birlikte yayınlanması gerekir.
8. Radyo kataloğunu geliştirip genişleterek daha zengin ve eksiksiz hale getirdik.


---

Sürüm 0.9.15 – 2026-10-14

1. Sesli Açıklamalar: Sözsüz seslendirmeler sırasında açıklamalara izin veren bir seçenek eklendi.

2. Seçenekler → Ses'teki sabit klavye gezinmesi: Sekme, kontroller arasında geçiş yapmanızı sağlar ve Esc, uyku zamanlayıcısı olsa bile pencereyi doğru şekilde kapatır.

3. Filmin ve seçilen sesin otomatik ön izlemesi ve son dışa aktarmada kullanılan aynı otomatik kısma özelliği ile Sesli Açıklama Oluşturma ve Sesli Açıklamayı Düzenleme Projesinde film ses düzeyi ayarı eklendi.

4. Sesli Açıklamaların Sesini Ayarla bölümüne ton birleşimi eklendi. Seçilen ton hatırlanır ve projelere kaydedilir.

---

Sürüm 0.9.14 – 2026-10-12

Edge Sesleri, Sesli Kitaplar ve Sesli Açıklamalar
1. Daha güvenilir Edge sesleri. Sonarpad artık metin okumak ve sesli kitap oluşturmak için en son Microsoft Edge kısayolunu kullanıyor. Eğer mevcut değilse otomatik olarak bir önceki linke geçebilir. Seçtiğiniz ses değişmez.

2. Daha az kesinti. Microsoft bir isteği reddederse veya bağlantı kesilirse Sonarpad, gerektiğinde bir süre bekleyerek kendi başına devam etmeye çalışır. Birkaç denemeden sonra durmaz ve istediğiniz zaman İptal'i seçebilirsiniz.

3. Cümle kesme yok. Sesin bir kısmı tam olarak ulaşmazsa, Sonarpad onu atar ve yalnızca o kısmı yeniden oluşturmaya çalışır. Zaten tamamlanmış çalışma korunur. Sesli kitaplar ve sesli açıklamalar, 8'e kadar eşzamanlı istekten yararlanmaya devam ediyor.

4. Daha net teşhis. Günlüklerde sesi gerçekte hangi Edge bağlantısının ürettiğini görebilirsiniz, böylece sorun olması durumunda neyin işe yaradığını anlamak daha kolaydır.

5. Daha düzenli ilerleme. Sesli açıklamaları kaydederken ve dışa aktarırken yüzde yalnızca değiştiğinde kaydedilir. Günlük artık aynı mesajlarla dolmaz ve ilerleme çubuğu normal şekilde çalışmaya devam eder.

---

Sürüm 0.9.13 – 2026-10-11

Ses ve okuma
1. Seçenekler → Ses'e 10 ila 120 dakika aralıklarla (cep telefonunda olduğu gibi) varsayılan olarak devre dışı bırakılan genel uyku zamanlayıcısı eklendi. Zamanlayıcı, yönetilen akışlar da dahil olmak üzere belge okumayı ve ses/video oynatmayı etkiler; süresi dolduğunda normal oynatıcı komutlarını kullanarak oturumu keser. Menülerde yeni komutlar olmadan yalnızca Ses Seçeneklerinden etkinleştirilir.



PDF belgeleri
2. Vurgulu ve Unicode karakterlerin (örneğin è, à ve é) PDF dışa aktarımı düzeltildi. Sonarpad artık Windows'ta zaten yüklü olan bir Unicode TrueType yazı tipini PDF'ye gömerek, yeniden açarken "è" gibi hatalı karakterlerin önüne geçiyor. Düzen, başlık ve sayfa numaraları değişmeden kalır.



Yapay zeka sesli açıklaması – kararlılık ve kurtarma
3. "Nesil_in_progress" sunucusundan gelen yanıtlar artık Sonarpad AI istemcisi tarafından "Yeniden Dene / İptal" mesajı yeniden önerilmeden normal bekleme olarak işleniyor. Doğrulama her zaman aynı önemsizlik anahtarını yeniden kullanır: halihazırda devam eden bir nesil ikinci kez başlatılmaz. Geçici ağ ve sunucu hataları, mevcut yönetime geri dönmeden önce sınırlı sayıda deneme için aynı anahtarla yeniden denenir. Gemini, segmentasyon, sentez, hizalama veya kredi hesaplaması değişmez.

4. Bir yüklemenin temizlenmesi, yalnızca sunucunun "401 geçersiz_session" yanıtı vermesi durumunda tek bir oturum yenilemesi kullanır. İstemci bu kurtarma sırasında yeni nesilleri başlatmaz ve terk edilen dosyaları arka ucun otomatik temizliğine bırakır.

5. Sahte 'OSError: Geçersiz argüman' mesajının önlenmesi amacıyla, Windows bir iptal sonrasında ilerleme kanalını kapattığında Python köprü sonlandırması sessiz hale getirildi. Kesilen bir sesli açıklamayı, kontrol noktalarını ve halihazırda tamamlanmış bölümleri devam ettirmeye yönelik mevcut işlev değişmeden kalır.

6. FFmpeg oynatıcısı için yalıtılmış bir geri dönüş eklendi: yalnızca bozuk ses paketlerinde tekrarlanan hatalardan sonra kod çözücü kontrollü bir kurtarma girişiminde bulunur ve akış kurtarılamazsa yüzlerce hatayı biriktirmek yerine okumayı bitirir. Normal kod çözme değişmez.

7. Sesli açıklama köprüsü önbelleği 'audio_description_bridge_v9.exe' olarak güncellendi: Sonarpad'i güncelledikten sonra, işlev ilk kez kullanıldığında köprü yeniden indirilir; mümkün olduğunda eski sürümlü önbellekler silinir. Gemini/Pyannote hattı değişmeden kalıyor. Yayımlamadan önce, güncellenmiş Python köprüsünü yeniden derleyin ve GitHub'daki Sonarpad-Tools'un 0.7 sürümündeki `audio_description_bridge.exe` varlığını değiştirin: aksi takdirde önceki sürüm yine de indirilecektir.

Oynatıcı, YouTube ve akış
8. YouTube videolarını MPV ile oynatırken tempoya gitme (Ctrl+T) düzeltildi: mevcut pencere artık bir BASS ses oynatıcısına ihtiyaç duymadan MPV yoluyla gerekli noktaya ulaşmanızı sağlıyor. Menü ayrıca MPV ile oynatılabilen web videoları için gezinme kontrollerini de etkinleştirir. Canlı TV ve doğrudan BASS akışlarının sınırlarının yanı sıra normal oynatma ve sesli açıklama hattı da değişmeden kalır.

9. Zamana git işlevi (Ctrl+T) ve ilgili menü komutları tüm isteğe bağlı içeriğe genişletildi: RaiPlay, Rai sesli açıklamaları, Sonarpad Sesli açıklamaları, La7 Play ve YouTube. Hizmet ilkinden farklı bir URL döndürse bile MPV artık etkin oynatıcıyı tanıyor; BASS tarafından yönetilen RaiPlay/La7 Play kayıtları mevcut zaman penceresini kullanabilir. TV kanalları ve canlı yayınlar kapsam dışındadır. Gemini boru hattında, FFmpeg'de veya sesli açıklamalarda değişiklik yok.

10. Yerel video dosyaları AVI, MP4, MKV ve *Jaws* gibi MPV ile oynatılan diğer formatlar için de Doğru Git zamanını (Ctrl+T) kullanın. Sonarpad, Windows yolunda büyük/küçük harf veya eğik çizgi farklılıkları olsa bile sekmeyi gerçek açık yerel dosyayla eşler. Mevcut pencere, BASS oynatıcı ve oynatma değişmeden kalır; TV kanalları ve canlı yayınlar harekete izin vermemeye devam ediyor. Sesli açıklama kanalında değişiklik yok.

11. Bazı *Jaws* AVI'ler gibi bozuk ses içeren videoların sesli açıklamaları için güçlendirilmiş FFmpeg geri dönüşü. Sonarpad ayrıca iyi paketlerle dönüşümlü olarak kötü paketleri de sayar ve anormal döngüleri durdurur. Ancak bundan sonra, zaten dahil olan FFmpeg kitaplıklarını (avformat, avcodec ve swresample) kullanarak kod çözmeyi yeniden dener, katı sınırlarla kısa bozuk aralıkları atlar ve tamamlanmamış WAV'leri reddeder. Ffmpeg.exe'ye veya başka herhangi bir program yüklemenize gerek yoktur. Normal Gemini/Pyannote hattı değişmeden kalıyor.

12. RaiPlay menüsünden Canlı bölümünü gizleyin. Talep üzerine film, dizi ve programlar mevcut olmaya devam edecek; Sonarpad'in ayrı TV bölümü değişmiyor.

PDF okuma ve Japonca OCR
13. Japonca kitapların İngilizce metinlerle açılması iyileştirildi. Taranan PDF'lerde Sonarpad, mevcut Windows OCR'yi kullanır ve Japonca OCR (ja-JP) yüklüyse, metni kopyalamadan konuma göre sonuçları birleştirerek hiragana, katakana ve kanji'yi de tanır. Satırlar ve güvenilir bir şekilde tanınabildiğinde sütunlar farklı kalır. Gömülü Japonca Unicode metin içeren PDF'ler artık farklı satırları keyfi olarak birleştirmiyor. Sayfa referansları kalır. Bulut hizmeti veya harici yürütülebilir dosya yok; normal PDF'ler ve sesli açıklama hattı değişmeden kalır.

14. Japonca OCR'nin yüklenmesi artık isteğe bağlıdır ve müdahalesizdir: Taranmış bir PDF açıldığında hiçbir uyarı görünmez. Hiragana, katakana veya kanjiye ihtiyaç duyanlar Araçlar > Okuma ve İçerik > PDF için Japonca OCR Yükle seçeneğini (veya gruplanmamış Araçlar menüsündeki ilgili öğeyi) seçebilirler. Sonarpad, bileşenin zaten mevcut olup olmadığını kontrol eder ve resmi Windows kurulumundan önce onay ister. Normal OCR ve sesli açıklama hattı değişmedi.

15. İsteğe bağlı Japonca OCR kurulumunda artık NVDA ve JAWS tarafından erişilebilen, İptal düğmesi bulunan gerçek bir bekleme penceresi var. İptalin onaylandığı ve sonucun veya gerekli yeniden başlatmanın açık bir şekilde belirtildiği PowerShell aracılığıyla resmi Windows kurulumunu kullanın; PDF'leri normal şekilde açarken hiçbir uyarı görünmüyor.

16. Sesli Açıklama projesinden MP3'leri yeniden dışa aktarırken, ilerleme çubuğu ve İptal düğmesi artık NVDA ve JAWS tarafından erişilebilen ayrı bir pencerededir. Proje düzenleme komutları artık gezinmeyi engellemiyor; işleme, iptal etme ve kaydetme işlemleri değişmeden kalır.

Sürüm 0.9.12 – 2026-10-02

Podcast kaydı
1. Bazı WASAPI sürücülerinde ses titremesine karşı uyarlanabilir ve koruyucu bir geri dönüş eklendi. Normal sistemlerde Sonarpad mevcut senkronizasyon yolunu tam olarak korur; yalnızca bir kayıt, yaklaşık 1 ms'den daha az zaman damgalarının kalıcı mikro titreşimini gösterdiğinde, her bir mikrofon paketinin gerilmesini veya sıkıştırılmasını önler ve bunun yerine bu paketleri süreklilik içinde korur. Daha büyük farklar, gerçek WASAPI süreksizlikleri ve zaman damgası hataları mevcut düzeltmeyi kullanmaya devam ediyor. Tanılama artık geri dönüşün tetiklenip tetiklenmediğini ve kaç tane mikro düzeltmenin önlendiğini gösteriyor.

AI ile sesli açıklama
1. Video akışının küçük bir tekrarlanan zaman farkıyla başladığı geçerli kayıtlar için koruyucu bir geri dönüş eklendi; bu, "geçersiz Gemini öbek zaman çizelgesi" hatasına neden olabilir. Sonarpad her zaman önce normal zaman çizelgesini ve halihazırda mevcut olan geri dönüşü drift için dener; ancak her ikisi de başarısız olursa ve parçalar küçük, tutarlı ve tekrarlanan bir başlangıç ​​zamanı gösterirse, bu ek yükü yalnızca etkilenen parçalarda normalleştirir. Tutarsız veya büyük dengelemeler reddedilmeye devam ediyor, bu nedenle zaten çalışan videolar normal boru hattını değişmeden takip ediyor. Tanılama artık zaman çizelgesi geçersiz kaldığında parçaların süresini ve başlangıç ​​zamanını da rapor ediyor.

2. Bazı TV/canlı kayıtlarda karşılaşılan başka bir 'geçersiz Gemini öbek zaman çizelgesi' durumu düzeltildi. FFmpeg tam olarak beklenen sayıda analiz parçası ürettiğinde ancak konteynerlerin küçük bir azınlığı mantıksız süreler bildirdiğinde, Sonarpad normal parçaların gerçek sürelerini korur, yalnızca bariz meta veri aykırı değerlerini segmentasyon tarafından tahmin edilen süre ile değiştirir ve en fazla %6'lık bir son yeniden düzenleme uygular. Geri çekilme yalnızca mevcut zaman çizelgesi yolları başarısız olduğunda devreye girer: normal videolar değişmeden kalır ve yaygın veya tutarsız zamanlama hataları olan kayıtlar reddedilmeye devam eder.

Oynatıcı, YouTube ve akış
1. Medya oynatımı sırasında odak izolasyonu düzeltildi. Yönetilen bir BASS veya MPV oynatıcı sekmesi etkin olduğunda, Tab/Shift+Tab artık gizli düzenleyici veya ses paneli kontrollerine giremez ve Space gibi oynatıcı kontrolleri, Akış gibi devre dışı bırakılmış yabancı kontrolleri etkinleştirmek yerine oynatmaya atanmış olarak kalır. Geçerli sekme bir oynatıcı olduğunda, normal düzenleyici sekmelerinin veya kalıcı pencerelerin davranışını değiştirmeden, düzenleyiciye yapılan ertelenmiş odak istekleri de göz ardı edilir.

2. Yapay zeka sesli açıklamasının önizlemesini izledikten sonra YouTube/MPV'ye dönme sorunu düzeltildi. Önizlemeyi kapatırken geçerli sekme hala aynı YouTube videosunu temsil ediyorsa Sonarpad tek bir ertelenmiş kontrol gerçekleştirir ve MPV'yi yalnızca ne MPV ne de BASS zaten etkin değilse yeniden başlatır. Bu, geçici "oyuncusuz oyuncu kartı" durumunu ortadan kaldırarak yinelenen işlemlerden veya yeniden başlatma döngülerinden kaçınır.

3. "Medyayı akıştan oynat" alanına yapıştırılan doğrudan YouTube bağlantılarının yönetimi iyileştirildi. Başlık halihazırda YouTube aramasından bilinmiyorsa, Sonarpad artık gerçek video başlığını yt-dlp yoluyla alıyor ve oynatıcı ve sekme için kullanıyor ve mümkün olduğunda "izle?v=..." adresi bırakmaktan kaçınıyor.

4. YouTube doğrudan oynatma/akışından indirilen dosya adları düzeltildi. Farklı Kaydetmeden önce, başlık eksikse YouTube Sonarpad onu tekrar kurtarmaya çalışır ve normalde videonun gerçek başlığını önerir. Başlık zaten mevcut değilse YouTube artık genel "stream_media" yerine "youtube_<video-id>" kullanıyor; "stream_media" yalnızca YouTube dışı akışlar için bir yedek olarak kalır.

5. Mix/Playlist parametrelerini içeren doğrudan YouTube bağlantılarına sahip sonsuz bir Esc döngüsü düzeltildi. Yt-dlp, gerçek bir koleksiyon veya arama katmanı açmadan böyle bir URL'den tek bir videoya geri dönerse, oynatmanın durdurulması artık aynı akışı yeniden açmak yerine düzenleyiciye geri döner. Gerçek arama, çalma listesi ve iç içe geçmiş koleksiyon katmanları daha önce olduğu gibi sıfırlanmaya devam ediyor.

6. Mix/Playlist parametrelerini de içeren YouTube videolarına doğrudan bağlantılar düzeltildi. Yapıştırılan bir URL açıkça "watch?v=<video-id>" içerdiğinde, Sonarpad artık bu video kimliğine "list=" ve "index=" yardımcı parametrelere göre öncelik verir, böylece bir YouTube Mix'in çekirdeği artık istenen videonun yerini alamaz. Doğru `/playlist?list=...` URL'leri bunun yerine koleksiyon olarak açılmaya devam eder.


Sürüm 0.9.11 – 2026-09-22

Dosyaları kaydetme
1. Sabit Farklı Kaydet: Halihazırda açık olan bir belge yeni bir adla kaydedildiğinde, kaydetme penceresi artık yapılandırılmış varsayılan klasörden veya OneDrive'dan başlamak yerine orijinal dosyayla aynı klasörde açılıyor. Yeni belgeler yapılandırılmış kaydetme klasörünü kullanmaya devam eder.

Google Sesleri
1. Google girişlerini başlatmaya güvenli bir geri dönüş eklendi: Chrome, Google TTS çalışma zamanı hazır olmadan önce kapanırsa Sonarpad, mümkün olduğunda Microsoft Edge ile otomatik olarak yeniden dener. Google Voices'ın halihazırda çalıştığı bilgisayarlarda Chrome'daki mevcut yol değişmeden kalır.

2. Google girişleri tarafından kullanılan nadir tarayıcı başlatma hataları için geliştirilmiş teşhisler: Chrome veya Edge stdout ve stderr, yalnızca Google TTS çalışma zamanı hazır olmadan önce tarayıcı kapatıldığında Sonarpad teşhislerine yazılır. Başarılı başlatmalar tanılamaya tarayıcı çıktısı eklemez.

Sonarpad Sesli Açıklamaları
1. En Son listesi, mobil cihazlarda halihazırda kullanılan aynı sunucu tarafı klasör gruplamasını kullanacak şekilde güncellendi. Yeni eklenen birden fazla bölüm aynı klasöre ait olduğunda, artık tüm bölümler yerine En Son'da klasör için yalnızca bir giriş görünüyor; açtığınızda içerik görüntülenir ve geri döndüğünüzde Son Kullanılanlar'a geri dönersiniz. Kataloğun tamamı ve normal klasör gezintisi değişmeden kalır.

2. Mobil cihazlarda olduğu gibi Windows'ta da Sonarpad Sesli Açıklamalar kataloğuna olay örgüsü okuma özelliği eklendi. Sunucu bir grafik sunduğunda, NVDA ve diğer ekran okuyucular bunu giriş başlığından hemen sonra okur; son zamanlarda tarih olay örgüsünden sonra kalır.

3. Sonarpad Sesli Açıklamalarında Esc ile RaiPlay ile tutarlı bir şekilde iyileştirilmiş navigasyon. Bir klasörün içinde Esc artık bağlamı koruyarak önceki düzeye dönüyor; tüm kataloğun kökünden Son Kullanılanlar'a döner, ana Son Kullanılanlar ekranından ise işlevi kapatır ve odağı düzenleyiciye döndürür.

Bu işlev için Sonarpad Audiodescriptions'a, filmleri depolamam için bana web alanı sağlayan Nicolò La Ferla'ya ve sesli anlatımlı filmlerin günlük olarak paylaşıldığı grubu yöneten Chiara Schiroli'ye özellikle teşekkür ederim.

Sürüm 0.9.10 – 2026-09-16

Sesli açıklamalar
1. Sonarpad ile sesli anlatımlı filmleri dinlemek için yeni Sonarpad Sesli Açıklamalar özelliği eklendi.

2. Rai sesli açıklamalarının açılması iyileştirildi: artık önceden indirilmiyor, hemen açılıyor. Kullanıcı bunları indirmek isterse, talep üzerine bunu her zaman yapabilir.

3. Metinden konuşmaya özelliğinin sesli açıklamalarda bu karakteri söylemesini önlemek için `_' (alt çizgi) karakterini bir boşlukla değiştirerek sesli açıklamaları iyileştirin.

Sürüm 0.9.9 – 2026-09-15

AI ile sesli açıklama
1. Halihazırda çalışan boru hattını değiştirmeden, ses parçası olmayan kaynak videolar için yalıtılmış bir geri dönüş eklendi. Sonarpad her zaman önce normal dışa aktarmayı dener; yalnızca FFmpeg ses akışının olmadığını bildirirse ve Sonarpad dosyanın herhangi bir ses parçası içermediğini onaylarsa, üzerinde sentezlenen açıklamaların karıştırıldığı aynı süreye sahip sessiz bir kaynak oluşturulur. Orijinal videonun üzerine dışa aktarırsanız normal, zaten mevcut olan video+ses çoklayıcı yolu yeniden kullanılır. Halihazırda ses içeren videolar tam olarak normal yolu izlemeye devam eder.

2. Normal sesli açıklama hattını değiştirmeden açıklamaların bulunmaması nedeniyle tamamen izole edilmiş geri dönüş genişletildi. Sonarpad her zaman önce mevcut analizi çalıştırır; ancak kullanılabilir açıklamalar olmadan sona erdiğinde, sessizlikler üzerindeki tüm kısıtlamaları ve çakışan diyalogların yasağını aktif tutarken aynı boru hattını kullanan Kısa modda ikinci bir girişim önerir. Bu ikinci deneme de bir açıklama eklemeyi başaramazsa, Sonarpad artık son bir geri dönüş için açık izin ister. Ancak Evet'i seçtikten sonra, Gemini tarafından halihazırda oluşturulmuş ve ikinci denemenin kontrol noktasına kaydedilmiş kısa açıklamaları yeniden kullanır ve bunları ilgili görsel zamanlarla karıştırıp diyaloglarla kısa örtüşmelere izin verir. Pyannote, Gemini köprüsü, normal hizalama kuralları, TTS zamanlaması ve ilk iki ayrıştırma yolu değişmedi. Kullanıcı reddederse veya yeniden kullanılacak oluşturulmuş bir açıklama yoksa Sonarpad, yerelleştirilmiş bir mesajla doğru şekilde sonlandırılır.

3. Normal sesli açıklama hattını değiştirmeden açıklama yapılmaması için iyileştirilmiş izole geri dönüş. Kullanıcı tarafından başlangıçta seçilen söz zaten Kısaysa ve analiz kullanılabilir açıklamalar olmadan sona eriyorsa, Sonarpad artık ikinci gereksiz Gemini analizini atlıyor ve oluşturulan açıklamalar yeniden kullanıma uygun olduğunda doğrudan diyalog katmanıyla son geri dönüşün kullanılıp kullanılmayacağını soruyor. Bununla birlikte, başlangıçtaki laf kalabalığı Normal veya Ayrıntılı ise, Kısa deneme, mevcut duraklamalar çok kısa olduğunda bile sürdürülür: bu durumda aynı zamanda herhangi bir son geri dönüşte kullanılacak daha kısa bir açıklama oluşturmaya da hizmet eder ve anlatımın diyaloğu kapsadığı süreyi kısaltır. Pyannote, Gemini köprüsü ve sessizlik ve hizalamayla ilgili normal kurallar değişmeden kalır.

Podcast kaydı
1. Halihazırda çalışan sistemlerde kaydı değiştirmeden, bazı sorunlu WASAPI sürücülerinde mikrofon cihazı konumu için muhafazakar bir geri dönüş eklendi. Geri dönüş yalnızca kalıcı cihaz konum uyumsuzluklarından sonra etkinleştirilir: 24 ardışık veya 64 temiz paketin en az %80'i. WASAPI'nin gerçek `DATA_DISCONTINUITY` ve zaman damgası hataları her zaman geçerli kalır ve sistem ses yakalaması etkilenmez.

YouTube ve akış
1. Sonuç içerik menüsünden başlayan başarısız YouTube indirme işlemleri düzeltildi. Yalnızca kanal üyelerine özel videolarla ilgili bilinen hatalar artık yt-dlp'deki ham İngilizce metni göstermek yerine Sonarpad'in yerelleştirilmiş mesajına dönüştürülüyor. Hatayı kapattıktan sonra Sonarpad ayrıca yerel menünün/modalın sonunda ertelenmiş odak sıfırlaması gerçekleştirir ve klavye etkinken sonuçlar listesine güvenilir bir şekilde geri döner. Normalde başarılı indirmelerin ardışık düzeni değişmeden kalır.

2. Önleyici filtre SonarTube mobile ile uyumlu hale getirildi: YouTube/yt-dlp'nin herhangi bir görüntüleme verisi döndürmediği videolar artık arama sonuçlarında ve kanal/oynatma listesi gezinmesinde gizlenirken, kanallar ve oynatma listeleri görünür kalır ve 0 görüntülemeye sahip gerçek bir video korunur. Yalnızca üyelere özel içerik için yerelleştirilmiş hata işleme, ikinci bir geri dönüş olarak etkin kalır. Çalma listelerinin çoklu indirilmesi etkilenmez.

Sürüm 0.9.8 – 2026-09-12

AI ile sesli açıklama
1. Halihazırda çalışan dosyaların işlem hattını değiştirmeden, çok kanallı parçalarla sesli açıklamaların dışa aktarımı konservatif olarak güçlendirildi. Sonarpad her zaman orijinal kanal düzeniyle devam eder; yalnızca çok kanallı ara WAV, kanal sayısına göre hizalanmayan bir çerçeve/örnek için başarısız olursa, yalnızca bu dışa aktarma stereo geri dönüşle tekrarlanır. Zaten doğru bir şekilde dışa aktarılan mono, stereo ve çok kanallı dosyalar normal yolu tam olarak takip eder.

Koyu tema ve erişilebilirlik
1. Koyu temanın yerel Windows pencerelerine ve onay mesajlarına müdahale etmesi düzeltildi. Sonarpad artık karanlık tema alt sınıfını "#32770" sistem iletişim ağacı ağaçlarına uygulamamaktadır: Pencereleri Aç/Kaydet, teşhis aktarımı ve Mesaj Kutuları Windows tarafından yönetilmeye devam eder ve klavyeyi ve NVDA odağını düzgün bir şekilde alabilir. Ayrıca Tanılamayı Kaydet penceresinin varsayılan ZIP uzantısıyla ilgili dahili bir sorun düzeltildi.

Sürüm 0.9.7 – 2026-09-11

AI ile sesli açıklama
1. İsteğe bağlı “Orijinal video dosyası üzerinde sesli açıklama oluştur” seçeneği eklendi. Etkinleştirildiğinde, Sonarpad artık MP4'ü tercih ediyor: orijinal video akışını yeniden kodlamadan kopyalar ve sesle açıklanan ses parçasını ekler. FFmpeg, MP4 oluşturma sırasında bir kapsayıcı veya paket yazma uyumsuzluğu bildirirse Sonarpad, videoyu yeniden kodlamadan otomatik olarak aynı MKV dışa aktarma işlemini yeniden dener. Ayrıca açıkça MKV'yi de seçebilirsiniz. Bu modda, ses ve videoyu senkronize tutmak için uzun duraklamalar göz ardı edilir.

2. Kaydedilmiş sesli tanımlayıcı projelerde geliştirilmiş segment yeniden analizi. Pencere artık ana Sesli Açıklama Oluştur penceresinde önceden yapılandırılmış olan AI erişimini kullanıyor, bu nedenle artık API anahtarını, AI Sonarpad kredisini ve şablon seçimini çoğaltmıyor. Yeniden analiz edilen bir segment artık yalnızca ilk/en yakın olan yerine söz konusu analiz parçasına ait tüm proje açıklamaları grubunu koruyor: gruptaki tüm açıklamalar yeniden analiz edilmiş olarak işaretleniyor ve "Segmenti uygula ve MP3'ü yeniden dışa aktar" tek bir işlemde tüm gruba uygulanıyor. Genişletilmiş duraklatma önizlemeleri, dışa aktarılan MP3'teki noktayı aramak yerine doğrudan sentezlenir; film sesiyle başlayan veya açıklamayı kesen önizlemelerden kaçınılır.

3. Halihazırda çalışan işlem hattını değiştirmeden Gemini'ye gönderilen medya için iyileştirilmiş koruyucu geri dönüş. HTTP 400 `INVALID_ARGUMENT` hatası, daha küçük MKV parçalarını (yaklaşık 15 MB) ve ardından uyumlu MP4 parçalarını ateşlemeye devam ediyor. Ayrıca, Gemini bir parçayı kabul ederse ancak işlenmesi "BAŞARISIZ" olarak biterse, Sonarpad tam olarak aynı parçayı bir kez yeniden yükler ve ancak bundan sonra MP4 geri dönüşüne geçer; Code 13 sunucusu için yeniden denemeler aynı geri dönüşten önce üç ile sınırlıdır, böylece sonsuz beklemeler önlenir. Ağ, kota, kimlik doğrulama veya sentez hataları bu yolu etkinleştirmez.

4. Oturum açma modları değiştirilirken AI kimlik bilgilerinin kalıcılığı düzeltildi. Kişisel Gemini API anahtarınız ve Sonarpad AI kodunuz artık bağımsız olarak kaydediliyor: Kişisel API Anahtarından Sonarpad AI'ye geçiş veya tam tersi, etkin olmayan kimlik bilgileri artık silinmiyor. Gemini anahtarı, ilgili alan odağı kaybettiğinde de kaydedilir.

5. "Segmenti Yeniden Analiz Et" ve "Segmenti Uygula ve MP3'ü Yeniden Dışa Aktar" için gerçek bir İptal düğmesi eklendi. İlerleme çubuğu etkin kalırken İptal artık segmentin FFmpeg hazırlığını, yapay zeka çalışanını, TTS doğrulamasını ve son dışa aktarımını mevcut aşama güvenli bir şekilde durdurulabildiği anda durdurur. Yeniden analiz edilen bir segmentin uygulanması artık işlemseldir: mevcut proje ve medya dosyası yalnızca başarılı bir yeniden dışa aktarma işleminden sonra değiştirilir; iptal ederseniz önceki dosyalar değişmeden kalır ve yeniden analiz edilen teklif yeni bir deneme için kullanılabilir durumda kalır.

6. İlk Gemini öbeğinin ötesindeki açıklamalar için segmentin yeniden ayrıştırılması düzeltildi. Yalıtılmış bölüm artık sıfırdan başlayan yerel bir zaman çizelgesiyle mevcut çalışana gönderilir ve ardından normal sesli açıklama ardışık düzenini değiştirmeden "Öbek 1'de geçersiz hazırlanmış yığın zaman çizelgesi"nden kaçınılarak dahili olarak projenin mutlak konumuna geri yönlendirilir.

7. Kaydedilen proje bölümlerinin yeniden analizi, tam sesli açıklama oluşturmayla aynı güvenlik kontrollerini kullanacak şekilde yeniden düzenlendi. Seçilen parça artık Pyannote ile aynı 16 kHz mono ses çıkarma ve diyalog/sessizlik analizini, aynı zamanlama kurallarını ve Gemini geri dönüşlerini, proje sesiyle aynı gerçek TTS sentezini ve güvenli konumlandırma ve uzun duraklamalar için aynı son zamanlayıcıyı gerçekleştiriyor. Segment uygulandığında Sonarpad, halihazırda doğrulanmış olan yeni mutlak süreleri korur ve bu yığının Pyannote korumasını günceller; Yeniden analiz edilen çok uzun metinlere yönelik önceki özel önizleme düzeltmesi kaldırıldı.

8. Tam yeniden analiz sonrasında segment yapısal değişimi düzeltildi. Sonarpad artık yeni doğrulanmış segmentin kaydedilen projeyle tam olarak aynı sayıda açıklama içermesini gerektirmiyor: bu parçanın eski açıklamaları, tüm ardışık düzenin tüm güvenli sonucuyla değiştirilir, böylece 10 açıklamaya sahip bir segment doğru şekilde 9 (veya 11) olabilir. Proje editörü önizleme için hemen yeni sayı ve süreleri gösterirken, kaydedilen proje ve medya dosyası "Segmenti uygula ve MP3'ü yeniden dışa aktar" başarıyla tamamlanana kadar değişmeden kalır.

9. Seçilen fiziksel parçanın gerçek bir bağımsız mini film olarak değerlendirilmesi için bölümün yeniden analizi yeniden düzenlendi. Sonarpad artık bu mini filmi doğrudan normal tam oluşturma tarafından kullanılan tamamen aynı "create_audio_description" işlevine aktarıyor, böylece video ve ses parçası aynı yerel zaman çizelgesini paylaşıyor ve normal Pyannote, Gemini, gerçek TTS, zamanlayıcı ve güvenlik kontrollerini yeniden ayrıştırmak için ayrı bir işlem hattı olmadan çalışıyor. Sadece normal boru hattının sonunda ortaya çıkan yerel zamanlar orijinal film konumuna geri döner.

10. Analiz edilen mini film orijinal filme kadar takip edildiğinde yeniden analiz süresi senkronizasyonu düzeltildi. Sonarpad artık tam analiz tarafından kullanılan aynı küçük parça süresi uzlaşmasını açıklamalara, Görsel Kanıt zaman damgalarına ve korumalı diyalog aralıklarına da uygulayarak parçanın sonuna doğru saniyenin onda birkaçı kadar artan bir boşluğun sesin konuşmanın üstüne çıkmasını önler.

Metni düzenle
1. Düzen > Metin'de "Kırık Çizgileri Birleştir" iyileştirildi. Seçim olmadan belgenin tamamı işlenir; bir seçim yapıldığında yalnızca seçilen satırlar etkilenir. Artık bir cümle noktalama işaretiyle bitse bile ardışık tüm satırları birleştirerek gerçek paragrafları yeniden oluşturuyor; boş satırlar paragraf ayırıcı olarak kalır ve numaralandırılmış veya madde işaretli listeler korunur.

Sürüm 0.9.6 – 2026-09-09

1. Aktif imleç hareketi sırasında bazı SAPI4 girişlerinin donması düzeltildi. Köprü artık sentezin fiilen tamamlanmasını bekleyerek imleç hareketini aktif tutar.

2. Mikrofon ile sistem sesi arasındaki senkronizasyon ve Kayıt Podcast'lerindeki zaman damgalarının yuvarlanmasından kaynaklanan tikler düzeltildi. Düzeltmeler kaynakların ayrı ayrı kaydedilmesinde de geçerlidir.

3. Sesli açıklamaların ayrı süreçler halinde izole edilmiş SAPI5 sentezi. Bir hata durumunda, başarılı açıklamalar korunarak sentez azaltılmış paralellik ile yeniden denenir; giriş için çalışma sınırı saklanır.

Sürüm 0.9.5 – 2026-09-07

AI ile sesli açıklama
1. Gemini geçici olarak hiçbir kullanılabilir içerik olmadan "MALFORMED_RESPONSE" döndürdüğünde dayanıklılık iyileştirildi. Sonarpad artık tüm sesli açıklamayı hemen kesintiye uğratmak yerine, denemeler arasında kısa bir bekleme süresiyle tam olarak aynı parçayı üç defaya kadar yeniden dener. Gemini'nin normal tepkileri, güvenlik kilitleri, jeton limiti hataları ve kontrol noktası davranışı değişmeden kalır.

2. FFmpeg tarafından hazırlanan parçaları küçük bir kümülatif süre farkı bildiren bazı videolarda nadir görülen bir başka "geçersiz Gemini parça zaman çizelgesi" durumu düzeltildi. Sonarpad, zaman çizelgesi geçerli olduğunda normal yolu değiştirmeden tutar ve yalnızca normal zaman çizelgesi başarısız olduğunda ve toplam sapma küçük olduğunda muhafazakar bir yeniden hizalama geri dönüşü kullanır; büyük veya şüpheli farklar yine de hata üretir.

3. Yapay zeka sesli açıklamaları için isteğe bağlı Sonarpad AI hizmeti eklendi. Kullanıcılar Gemini API anahtarlarını kullanmaya devam edebilir veya Sonarpad hizmetini seçip kişisel bir Sonarpad kodunu girebilirler. Kod Windows DPAPI ile korunmaktadır. Hizmet modunda, hazırlanan video parçaları geçici bir Google yükleme URL'si aracılığıyla doğrudan kullanıcının bilgisayarından Google'a yüklenir; sonarpad.com video baytlarını almaz veya saklamaz. Arka uç yalnızca erişime izin verir, Gemini/Standart modeli ve kullanım sınırlarını uygular, tüketimi hesaplar ve Gemini'nin yanıtını döndürür.

4. Kaydedilen sesli tanımlayıcı projelerin düzenlenmesi iyileştirildi. Artık her değişikliği hemen uygulamanıza gerek kalmadan birden çok açıklamayı birbiri ardına değiştirebilirsiniz. Değişiklikler hafızada kalır; “Metni Uygula” düğmesine basıldığında Sonarpad, değiştirilen her açıklamayı kayıtlı aralığına göre kontrol eder ve ardından tüm geçerli değişiklikleri birlikte kaydeder. Bir açıklama aralığına uymuyorsa odak, hâlâ uygulanacak diğer değişiklikleri kaybetmeden bu açıklamaya geri döner.

5. "Sesli Açıklama Oluştur"un hemen önüne "Sesi Ayarla" eklendi. Yeni pencere motoru, sesi, hızı ve ses seviyesini seçmenize olanak tanır ve "Test sesi"ni içerir. Tamam'a basıldığında odak tam olarak "Sesi ayarla" seçeneğine döner ve seçilen değerler oluşturulacak sesli açıklamaya uygulanır. Pencere hiç kullanılmazsa özet tam olarak eskisi gibi kalır.


6. Kaydedilmiş sesli tanımlayıcı projelerde genişletilmiş ses kontrolleri. Projeyi Düzenle'de artık motor ve sesin yanı sıra hız, ses seviyesi ve "Test Sesi" de mevcuttur. Sonarpad, ses parametrelerini uygulayarak mevcut tüm açıklamaları yeni sentez parametreleriyle yeniden kontrol eder ve MP3'ü yalnızca hepsi ilgili kaydedilmiş aralıklara uymaya devam ederse yeniden oluşturur. Diyalogsuz aralıklar, Görsel Kanıt süreleri, Gemini açıklamaları ve proje zaman analizi, AI analizini tekrar çalıştırmadan yeniden kullanılır.

Sürüm 0.9.4 – 2026-09-04

AI ile sesli açıklama
1. Alışılmışın dışında yüksek dahili başlangıç zaman damgası içeren Matroska/WebM videolarında "geçersiz Gemini öbek zaman çizelgesi" hatasıyla sesli açıklama oluşturulmasını durdurabilen bir sorun düzeltildi. Sonarpad artık kaynak dosyanın ve Gemini parçalarının süresini yalnızca bu anormal zaman damgası modelini algıladığında normalleştiriyor, normal videoları ve zaten çalışmakta olan sesli açıklama davranışını değiştirmeden bırakıyor.
2. Daha doğal bir miksaj için iyileştirilmiş ses açıklaması eğilmesi. Orijinal ses artık ses başlamadan önce düzgün bir şekilde düşüyor ve açıklama sonrasında yavaş yavaş normal seviyeye dönüyor; çok yakın açıklamalar aynı zamanda sabit bir arka plan seviyesini koruyarak sürekli düşüş ve yükselişlerden kaçınır.
3. Kod çözme tamamlanmamış bir geçmeli ses çerçevesiyle sona erdiğinde, bazı AVI dosyalarında ve 5.1/çok kanallı sese sahip diğer kaynaklarda son ses açıklaması aktarımının başarısız olmasına neden olabilecek bir sorun düzeltildi. Sonarpad artık yalnızca son kısmi kareyi kesinlikle gerekli sessiz örneklerle güvenli bir şekilde tamamlıyor; zaten tamamlanmış kareler ve normal mono, stereo veya çok kanallı dosyalar değişmeden kalır.


Sürüm 0.9.3 – 2026-09-03

SAPI5 girişleri
1. Çoklu sesli okuma sırasında veya sesli kitap/MP3 oluştururken bazı yerel SAPI5 seslerinin aktif imleç hareketi ile konuşamaması sorunu düzeltildi. Sonarpad artık dosyaya yönelik Windows'ta güvenilir olan ve sentez sırasında geçersiz kılınabilen bir SAPI5 sentez yolu kullanıyor; normal doğrudan okuma ise değişmeden kalıyor.
2. Çoklu sesli diyalogları okurken imleç konumu düzeltildi. Diyaloglar için Sonarpad tarafından otomatik olarak eklenen ses etiketleri artık yalnızca oynatma meta verileri olarak kabul ediliyor ve artık düzenleyicide mevcut karakterler yok, bu da imlecin F4 veya F6'dan sonra gerçek metnin önüne atlamasını engelliyor. Tek sesli okuma ve belgelerde açıkça yazılan <voice> etiketleri değişmeden kalır.

AI ile sesli açıklama
1. Gemini API Anahtarı alanının hemen sonrasına “API Anahtarını Göster” kutusu eklendi. Varsayılan olarak devre dışıdır; Etkinleştirildiğinde geçici olarak tam anahtarı gösterir, böylece tam olarak yapıştırıldığını doğrulayabilirsiniz. Pencere yeniden açıldığında anahtar her zaman tekrar gizlenir.

Podcast'ler ve Vikipedi
1. Bir podcast kaydını kaydettikten sonra Sonarpad, YouTube/akış ortamını kaydettikten sonra zaten yaptığı gibi, artık kaydedilen dosyayı içeren klasörün açılıp açılmayacağını sorar.
2. Halihazırda metin içeren bir düzenleyiciye başka bir Vikipedi makalesi aktarırken, yeni makale artık üst tarafa eklemek yerine alt tarafa ekleniyor. İmleç yeni içe aktarılan yeni makalenin başlangıcına konumlandırılır.


Sürüm 0.9.2 – 2026-09-02

AI ile sesli açıklama
1. 5.1 gibi çok kanallı ses içeren videolarda MP3'e son aktarım sırasında ses açıklamasının başarısız olmasına neden olabilecek bir sorun düzeltildi. Sonarpad artık mono veya stereo aktarımları etkilemeden çok kanallı sesi yalnızca MP3 kodlaması için gerektiğinde otomatik olarak stereoya dönüştürüyor.
2. Birden fazla ses parçası içeren bir videoyla AI Sesli Açıklama Oluştur'u başlattığınızda, Sonarpad artık başlamadan önce hangi parçanın kullanılacağını soruyor. Erişilebilir açılan kutu oklarla değiştirilebilir; Tamam, seçilen parçayı kullanarak sesli açıklamayı başlatır, İptal ise sesli açıklama penceresini kapatır ve odağı Sonarpad düzenleyicisine döndürür.

YouTube ve akış
1. Bir oynatma listesinin veya YouTube kanalının 2. veya sonraki sayfasındaki bir videodan Yapay Zeka ile Sesli Açıklama Oluştur'un başlatılmasının, YouTube seçim penceresini yeniden açarak odağı sesli açıklama penceresinden uzaklaştırmasına neden olan sorun düzeltildi. Sonarpad artık önceki sayfalara dönmeden seçiciyi düzgün bir şekilde kapatıyor.
Sürüm 0.9.1 – 2026-09-01

YouTube'u indirin
• Alt+Tab ile başka bir uygulamaya geçiş yapıldıktan sonra YouTube/yayın indirme ilerleme durumu pencerelerinin tekrar tekrar öne dönebilmesi sorunu düzeltildi. İndirmeler artık odağı bozmadan arka planda devam ediyor.
• İndirme işleminin erişilebilirliği iyileştirildi. İlerleme penceresine döndüğünüzde ekran okuyucular mevcut durumu ve yüzdeyi okuyabilir. Sonarpad çalma listelerinde ayrıca geçerli öğe numarasını, toplam öğeyi ve başlığı da gösterir.
• Uzun indirmeler ve dönüşümler sırasında, ilerleme penceresinin yanıt vermeye devam etmesi sırasında ortaya çıkan yanlış bekçi köpeği takılma tespiti düzeltildi.
• Çalma listesi indirme işlemine Format açılan kutusu eklendi. Toplu indirmeye başlamadan önce video listesinden Sekme tuşuna basmak MP4, MP3, M4A, OPUS, OGG, WAV veya FLAC'ı seçmenizi sağlar.
• Akış ortamının kaydedilmesi yeniden düzenlendi. Format ve kalite artık ilk akış arama penceresi yerine kaydetme sırasında seçiliyor. “Medyayı Kaydet”, Format ve Kaliteyi içeren tek bir pencere açar ve çalma listelerinde her iki birleşik giriş kutusu da mevcuttur.

AI ile sesli açıklama
• Yapay zeka sesli açıklamasının bazı MKV videolarıyla başlamasını engelleyebilecek bir sorun düzeltildi. Sonarpad artık düzensiz veya eksik zaman damgalarına sahip videoları daha güvenilir bir şekilde işliyor.

Sürüm 0.9.0 – 2026-08-31

AI Sesli Açıklama — yeni ana özellik
• Araçlar > Multimedya'ya “Yapay Zeka ile sesli açıklama oluştur” modülü eklendi. Sonarpad, diyalogsuz alanları belirlemek için sesi analiz eder, Gemini ile açıklamalar oluşturur ve satırlar üzerinden konuşmaktan kaçınarak programda zaten mevcut olan ses motorlarını kullanır.
• Gemini tarafından oluşturulan zamanlama üzerindeki otomatik kontrollerle, videoda olanlar ile açıklamalar arasında iyileştirilmiş senkronizasyon.
• Varsayılan olarak "Uzatılmış duraklamaları etkinleştir" seçimi kaldırılmıştır. Aksi takdirde çok uzun olacak açıklamaların eklenmesine izin vermek için çok fazla diyalog veya az alan bulunan içeriklerde etkinleştirilebilir.
• Karakterleri tanımayı ve isimlerini kullanmayı deneyebilirsiniz. Sürekliliği artırmak için bir dizinin bölümleri arasında karakter katalogları tutulabilir.
• Gemini ile her şeyi yeniden oluşturmanıza gerek kalmadan projeyi kaydedebilir, açıklamaları daha sonra düzenleyebilir ve yeniden dışa aktarabilirsiniz.
• Çalışmanız kesintiye uğrarsa Sonarpad ilerlemenizi korur ve sesli açıklamaya devam etmenize olanak tanır. Gemini kotası dolmuşsa, yapılan işi kaybetmeden beklemek, model değiştirmek veya ara vermek mümkündür.
• Pencere dili, ayrıntı düzeyini, Gemini modelini, motoru ve sesi seçmenizi sağlar ve kullanılan tercihleri hatırlar.
• Modül 17 Sonarpad dilinde mevcuttur. Oluşturma sırasında arayüz yalnızca ilerlemeyi, mevcut durumu ve İptal düğmesini gösterir; sonunda MP3 doğrudan dahili oynatıcıda açılabilir.
• Akış yoluyla açılan videolar ve RaiPlay ve La7 Play'in uyumlu isteğe bağlı içerikleri için "Yapay Zeka ile sesli açıklama oluşturun" seçeneği de mevcuttur. Bu menülerden fonksiyon açıldığında veya bir TV kaydının sonunda odak yönetimi de geliştirildi.

La7 Oyun ve multimedya
• Araçlar > Multimedya'ya La7 Play'in eklenmesi, program arama, La7 ve La7 Sinema canlı yayınları ve dahili oynatıcıda korumasız isteğe bağlı içeriğin oynatılması.
• İndirilebilir La7 içeriğinde bağlamsal menü artık medyayı kaydet, Metne dönüştür ve AI ile sesli açıklama oluştur seçeneklerini sunuyor. Medyayı Kaydet, MP3 ve MP4 arasında seçim yapmanızı sağlar.
• YouTube/streaming, RaiPlay ve La7 Play'in bağlamsal eylemlerinde iyileştirilmiş odak yönetimi, böylece indirme, transkripsiyon ve sesli açıklama pencereleri ön planda doğru şekilde kalır.
• Rai sesli açıklamalar modülünde, Medyayı kaydet artık iptal etme olasılığı olan bir ilerleme penceresi gösteriyor ve Transcribe, ilerleme penceresini tamamlanana kadar görünür tutuyor.
• Onay talebiyle birlikte TV ve radyo kayıtlarını silmek için Del tuşu eklendi.

E-kitaplar ve belgeler
• Düzenleyici ve dizinde mevcut metin ve bölümlerle birlikte MOBI, AZW ve AZW3 formatlarında DRM'siz Kindle'ların içe aktarılması eklendi.
• PAPATYA 2.02 ve PAPATYA 3 desteği eklendi. PAPATYA sesli kitapları Sonarpad'in dahili oynatıcısını kullanır ve gezinme ve bölüm sınırlarına uyar.
• Kindle ve PAPATYA, orijinal dosyanın üzerine yazılmadan içe aktarılır; DRM korumalı Kindle'lar açıkça reddedilir.
• EPUB'lar için "Farklı kaydet" seçeneği düzeltildi: TXT veya başka bir format seçildiğinde artık seçilen uzantı kullanılıyor ve orijinal EPUB açık belgeyle ilişkili kalıyor.

RSS ve makaleler
• Tek bir işlemde birden fazla RSS makalesinin silinmesi için birden fazla RSS makalesi seçimi eklendi.
• RSS artık boş klasörler de dahil olmak üzere OPML içe ve dışa aktarma sırasında korunan gerçek klasörleri de destekliyor.
• Geçerli klasör içindeki akışlar Yukarı Taşı, Aşağı Taşı, Yukarıya Taşı, Alta Taşı ve Konuma Taşı komutlarıyla yeniden sıralanabilir.

Erişilebilirlik, kılavuzlar ve arayüz
• TV Rehberi artık programları seçilebilir bir açılır kutuda gösteriyor ve seçilen programın konusunu görüntülemenize olanak tanıyor.
• Sonarpad kılavuzları bir indeksle yeniden düzenlendi ve Yapay Zeka Sesli Açıklama özelliğine kapsamlı bir kılavuz eklendi.
• Aç, Farklı Kaydet ve diğer dosya seçim pencerelerinin görünmesini engelleyebilecek Almanca çeviri sorunu düzeltildi.

Sesler ve diller
• İndirilebilir Google TTS sesleri kataloğu 104 paketten 156 pakete ve 53'ten 81 dil çeşidine çıkar.
• Tüm arayüz dillerinde yeni Google TTS paketleri ve diğer dillerin yerelleştirilmiş adları eklendi.

Sürüm 0.8.4 – 2026-07-24

EPUB belgelerini düzenleme
• Sonarpad artık yalnızca EPUB belgelerini açmakla kalmıyor, aynı zamanda orijinal formatı, içindekiler tablosunu, dipnotları, resimleri, stil sayfalarını, meta verileri ve dahili bağlantıları koruyarak bunları EPUB formatında düzenleyip yeniden kaydedebiliyor.
• “Farklı Kaydet” seçeneğinde EPUB formatı, EPUB'dan açılan belgeler için kullanılabilir. Kaydetme yalnızca değiştirilen metni günceller ve kitabın yapısını olduğu gibi korur.

Sesli açıklama içeren kayıtlar
• Sesle açıklanan parçayla sabit kayıtlar. Sonarpad artık orijinal videoyu, sesle açıklanan kanalların ses parçalarıyla birlikte doğru şekilde kaydedebiliyor.

Sesli kitapların güvenilirliği
• Beş başarısız Google TTS denemesinden sonra, bir sentez biriminin sessizce silinmesine ve son sesli kitapta bazı metinlerin eksik olmasına neden olan aralıklı bir sorun düzeltildi.
• Google Drive'lar artık başarılı olana veya kullanıcı iptal edene kadar yeniden denenecek. Chrome ve dosyalarla ilgili geçici çakışmaları azaltmak için işlem başlatma işlemleri kademeli olarak gerçekleştirilir; Sonarpad ayrıca bir bölümü eksik olan sesli kitabı kaydetmek yerine oluşturmayı da durdurur.
• Edge sesli kitapları ayrıca karışık girişler ve süreye göre bölme dahil olmak üzere, başarılı olana veya kullanıcı tarafından iptal edilene kadar geçici ağı, WebSocket'i, zaman aşımını, hizmet kısıtlamayı ve geçersiz ses hatalarını artık sınırsız olarak yeniden dener. SAPI4 ve SAPI5 uyarlanabilir ve sonlu denemeleri sürdürüyor; bir bölüm başarısız olmaya devam ederse Sonarpad, tamamlanmamış bir sesli kitabı kaydetmeden işlemi iptal eder.

Dijital kütüphanelere göz atma
• LibriVox, Internet Archive ve Project Gutenberg sonuçları artık YouTube gibi sayfalarda gezinmeyi kullanıyor: Listenin en üstünde "Önceki sonuçlara git" ve sonunda "Sonraki sonuçlara git" görünür.
• LibriVox'ta sabit odak değiştirme: Bir kitap veya bölüm açıldığında, NVDA artık bir sonraki listeyi veya okuyucuyu açmadan önce ana düzenleyiciye taşınmıyor.
• LibriVox kitaplarını ararken ve yüklerken odak koruması eklendi: yerelleştirilmiş bir yükleme penceresi istek boyunca ön planda kalır ve NVDA'nın odağının Komut İstemi, Windows Terminali veya başka bir uygulamaya geçmesini engeller.

YouTube oynatma listelerini indirin
• YouTube oynatma listelerine erişilebilir bir çoklu seçim komutu eklendi; bu komut, oynatılmakta olan öğeyle ilgili "Medyayı kaydet" komutunu değiştirmeden hangi videoların indirileceğini seçmenize olanak tanıyor.
• Seçilen öğeler, çalma listesi açılırken seçilen format ve kalite kullanılarak teker teker indirilir, orijinal sırayı koruyan numaralı adlar alınır ve yapılandırılmış Medya klasörü içindeki özel bir klasöre kaydedilir.
• "Tümünü Seç" ve "Tüm Seçimleri Kaldır" penceresini içeren pencere, kaç öğenin seçildiğini duyurur, tamamlanmış dosyaları korurken iptal etmenize olanak tanır ve indirilemeyen öğeleri açıkça belirtir.
• Çalma listesi öğeleri artık gerçek yerel onay kutularıdır: NVDA ve diğer ekran okuyucular, başlıkta ek kelimeler göstermeden ve zorunlu sesli duyurular olmadan başlığı, kontrol türünü ve etkin veya devre dışı durumunu otomatik olarak duyurur.

Sürüm 0.8.3 – 2026-07-23

Karanlık mod
• Görünüm menüsünden etkinleştirilebilen ve tercihlere kaydedilebilen karanlık mod eklendi.
• Koyu tema, Sonarpad'in düzenleyicisine, menülerine, alt pencerelerine ve ana kontrollerine uygulanarak metin renklerinin okunabilirliği ve erişilebilirliği koruyacak şekilde uyarlanması sağlanır.

Alman dili
• Seçenekler'den seçilebilen tam Almanca dili eklendi.
• Haberler ve RSS, yazım denetleyici, takvim ve tüm alıntılar, bağışlar, yardım ve değişiklik günlüğü tamamen Almanca olarak mevcuttur.

Brezilya Portekizcesi ve Google Haberler
• Brezilya Portekizcesi, Portekizce Portekizce'den ayrı ve Seçenekler'den seçilebilen tam arayüz dili olarak eklendi.
• Arayüz, takvim ve alıntılar, yazım denetleyicisi, bağışlar, yardım ve değişiklik günlüğü tamamen Brezilya Portekizcesinde mevcuttur.
• Google Haberler artık Brezilya yerelleştirmesini, Brezilya kategorilerini ve ayrı varsayılan Brezilya RSS kaynaklarını destekliyor.
• Akış bunları sağladığında, aynı haberle ilgili farklı Google Haberler kaynakları ağaçta erişilebilir alt öğeler olarak gösterilir.

Canlı TV
• RaiPlay'de olduğu gibi, canlı TV Rai kanallarını oynatırken Sonarpad artık mümkün olduğunda ses açıklanan parçayı otomatik olarak seçmeye çalışıyor.
• TV'nin daha hızlı açılması: yayın programları yalnızca görüntülenen kategori, bölge, arama veya favoriler sayfası için yüklenir ve geçici olarak yeniden kullanılır.
• Bir kategoriyi açmak için Enter veya Sağ Ok tuşuna basıldığında ilk kanalın yayılıp hemen başlatılmasına neden olan sorun düzeltildi.

LibriVox
• Hizmete aşırı talep gelmesini ve arayüz çökmelerini önlemek için LibriVox araması optimize edildi. Kapsamlı katalog taramaları ortadan kaldırıldı, yeniden denemeler azaltıldı ve maksimum süreler kısaltıldı.

Konuşma sentezi
• Üç veya daha fazla noktadan oluşan diziler artık okumadan önce normalleştirilerek bazı girişlerin "nokta nokta" olarak telaffuz edilmesi veya yalnızca noktalama işaretlerinden oluşan bölümler oluşturulması engelleniyor.

Google Haberler'den ilgili makaleler
• Artık her haber için, mümkün olduğunda ilgili makaleler, yani aynı haberle ilgili diğer makaleler gösteriliyor. Bunları okumak için Sonarpad ilgili makalelerin mevcut olduğunu bildirdiğinde ana makaleyi genişletmeniz yeterlidir. Bu bölümü genişletmek istemeyenler için ana yazıda Enter tuşuna basıp her zaman yaptığınız gibi haberi okumanız yeterli.
• İlgili makaleler artık erişilebilir duyuru, zaman damgası, durum kaydetme ve kaynaklar yenilendikten veya Sonarpad yeniden başlatıldıktan sonra durum saklama da dahil olmak üzere ana makalelerle aynı okuma/okumama sistemini kullanıyor.

Sesli kitapların bazı kısımlarındaki reklamlar
• Ses Seçenekleri'ne “Her bölümün başına duyuru” açılır kutusu eklendi. Birden fazla dosyaya bölünmüş sesli kitaplarda herhangi bir duyuru eklenmemesi veya her bölümün başında kitabın başlığının, parça numarasıyla birlikte başlığın, dosya adının veya parça numarasıyla birlikte dosya adının okunması da mümkündür.

Sürüm 0.8.2 – 2026-07-17

Dijital kütüphaneler ve sesli kitaplar
• Başlığa veya yazara göre arama ve dil seçimiyle Gutenberg Projesi eklendi.
• Project Gutenberg EPUB kitapları Documents\Sonarpad\Documents klasörüne indirilir; sonunda Sonarpad, kitabın editörde hemen açılıp açılmayacağını sorar.
• Eski radyo, konuşmalar ve canlı müzik dahil olmak üzere ses koleksiyonlarını aramak ve dinlemek için İnternet Arşivi eklendi.
• Sesli kitapları başlığa veya yazara göre aramak ve bölümleri podcast'ler için kullanılan aynı oynatıcıyla doğrudan oynatmak için LibriVox eklendi.
• Üç yeni işlev Araçlar menüsünde ve menü gruplaması etkin olduğunda Okuma bölümünde mevcuttur.

Uzun ses transkripsiyonları
• Uzun ses dosyalarının transkripsiyonu düzeltildi: Ses artık otomatik olarak 15 dakikalık bölümlere bölünüyor, her seferinde bir bölüm yazıya geçiriliyor ve sonra yeniden birleştiriliyor; böylece uzun dosyalarda oluşabilecek hatalar önleniyor.

YouTube
• Daha önce yalnızca bir YouTube videosunu açtıktan ve Oynat menüsünü kullandıktan sonra kullanılabilen en kullanışlı eylemler, artık daha fazla kullanım kolaylığı için "Geçerli sesi metne dönüştür", "Yapay zeka sesli açıklaması oluştur" ve "Medyayı kaydet" gibi doğrudan aynı videonun içerik menüsünden de kullanılabiliyor.
• Seçilen videonun, oynatma listesinin veya YouTube kanalının URL'sini panoya kopyalamak için Ctrl+C tuşlarıyla da etkinleştirilebilen "Bağlantıyı kopyala" öğesi eklendi.

Sürüm 0.8.1 – 2026-07-16

Google metinden konuşmaya
• Tarayıcının dahili sunucusu tarafından kabul edilen bağlantıların engellenmeyen yuva modunu devraldığı, 10035 hatasına neden olduğu ve indirilen seslerin konuşmasını engellediği Windows sistemlerinde Google TTS başlatma sorunu düzeltildi.
• Sonarpad artık F5 ile konuşmayı önizlemeden veya okumadan önce Chrome veya Edge WASM motorunun gerçekten yüklenmesini bekliyor ve "Chrome WASM TTS motoru yüklenmedi" hatasını önlüyor.
• Görünmez tarayıcıda sayfa çevirisi ve oluşturucu erişilebilirliği devre dışı bırakılarak “Sayfayı çevir” gibi duyurular ve okuma komutlarına müdahale önlenir.
• Google motoru seçildiğinde artık “Editör girişleri” panelinde “Google girişlerini yönet…” düğmesi görünüyor; yönetim kapatıldığında kurulu öğelerin listesi anında güncellenir.
• Google Konuşma paketleri kaldırılırken gösterilen bağımlılık uyarıları artık tüm arayüz dillerine çevrildi.

Yükseltme deneyimi
• Otomatik güncellemeden sonra değişiklik günlüğünü içeren tamamlama penceresi, ilk odak sıfırlamasının ardından açılır ve yalnızca Sekme tuşuna bastıktan sonra görünmek yerine ön planda kalır.

PDF belgeleri
• Gömülü metnin NUL karakterler içerdiği ve düzenleyiciye yüklenirken ilk geçişte kesildiği PDF'ler düzeltildi.
• pdf-extract gömülü NUL'ları döndürürse, Sonarpad PDFium'u yeniden dener; metni Windows kontrollerine göndermeden önce kalan NUL'lar kaldırılır ve belgenin geri kalanı korunur.

Menü erişilebilirliği
• Yürütme sırasında anımsatıcı hesaplaması kaldırıldı: erişim mektupları artık 15 arayüz çevirisinin her birine açıkça yazılıyor ve bu nedenle her başlangıçta aynı kalıyor.
• Oynatma, Yazı Tipleri, Görüntüyü Kaydet ve EPUB Dizinini Göster dahil olmak üzere ana menüler ve alt menülerdeki tüm sabit öğeler revize edildi; Kardeş girişler arasındaki eksik veya yinelenen anımsatıcılar doğrudan çevirilerde düzeltildi.
• Otomatik testler artık yalnızca çevirileri kontrol ediyor ve bir anımsatıcı eksik, geçersiz veya kopyalanmışsa başarısız oluyor; artık yürütme sırasında etiketleri değiştirmiyorlar.
• Olağanüstü büyük menülerde, çevrilen metin yeterli sayıda farklı karakter içermediğinde, standart Windows biçiminde “(&1)” açık bir sayısal erişim harfi gösterilir.

Sürüm 0.8.0 – 2026-07-15

Çevrimiçi sözlük
• Almanca dili Vikisözlük çevrimiçi sözlüğüne eklendi.
• Almanca tanımlar ve eşanlamlılar artık Almanca Vikisözlük'ün özel yapısına göre doğru bir şekilde tanınıyor.

Treccani Ansiklopedisi
• Treccani Ansiklopedisi'ndeki girdileri aramak ve okumak için İtalyanca arayüze yeni bir işlev eklendi.
• Bir sonucu seçebilir, makalenin tamamını veya tek bir bölümünü okuyabilirsiniz.
• Metni düzenleyicide açtıktan sonra Esc tuşuna basmak sizi Wikipedia işlevinde olduğu gibi bölüm seçim kutusuna geri götürür.
• Makale ayrıştırıcı düzeltildi: "Makalenin tamamını okuyun" seçeneği artık Treccani dizini ve içeriği ayrı HTML kapsayıcılarına yerleştirdiğinde bile makalenin tüm gövdesini kurtarıyor.
• Treccani Ansiklopedisi'ni hızla açmak için Ctrl+Shift+E kısayolu eklendi.

SAPI5 sesli kitapların güvenilirliği
• SAPI5 sesli kitap oluşturma, seçilen ses güvenilir sonuçlar ürettiğinde 12'ye kadar paralel çalışan kullanmaya devam eder.
• Oluşturulan her parça artık dosya boyutuna, tahmini süreye ve atanan metinle ihtiyatlı karşılaştırmaya göre kontrol ediliyor.
• Eksik veya şüpheli parçalar eşzamanlılık kademeli olarak azaltılarak otomatik olarak yenilenir: 12, 8, 6, 4, 2 ve son olarak 1 işçi. Sadece sorunlu kısımlar tekrarlanıyor.
• 12 çalışanla sorunsuz çalışan girişleri yavaşlatmadan, her SAPI5 girişi için güvenilir limit ayrı ayrı hatırlanır.
• Son bir kontrol, Sonarpad'in oluşturulan parçalardan çok daha kısa olan bir MP3'ü sessizce kabul etmesini önler.
• Oluşturma ayrıntıları ve tüm girişimler 'sapi5_audiobook_diagnostic.log' dosyasına kaydedilir.
• Artık her SAPI5 sentez birimi ayrı, görünmez bir Sonarpad sürecinde çalışıyor. Üçüncü taraf girişinin çökmesi durumunda yalnızca o çalışan kapanır ve ana uygulama açık kalır.
• Sesli kitap oluşturmanın kendisinde, tamamlanmamış kısımlar hemen bir sonraki en düşük eşzamanlılık düzeyiyle yeniden denenir; Halihazırda doğrulanmış olan parçalar korunur.
• Kurtarmayı yeniden başlatma, yalnızca ana uygulama veya bilgisayar durdurulduğunda ek koruma olarak kalır.

SAPI4 sesli kitap çalışanı
• Kullanıcı tarafından seçilen SAPI4 işlemlerinin sayısına artık teknik maksimum 64'e kadar uyulmaktadır; önceki 16 gizli sınırı kaldırıldı.
• Gerçek sayı yalnızca sesli kitap gerekli işlemlerden daha az iş birimi içerdiğinde azaltılır.
• Bir veya daha fazla SAPI4 köprü işlemi başarısız olursa, tamamlanan parçalar korunur ve yalnızca başarısız olan birimler giderek daha düşük eşzamanlılıkla otomatik olarak yeniden denenir.
• Sonarpad artık SAPI4 köprüsünün çıkış kodunu kontrol ediyor ve tamamlanmış saymak yerine boş veya geçersiz ses parçalarını reddediyor.

Proxy yapılandırması
• Ağ ayarlarına proxy bağlantı noktası için ayrı bir alan eklendi.
• Bağlantı noktası artık adresten bağımsız olarak gösterilebilir, 1 ila 65535 aralığında doğrulanır ve URL'de zaten mevcut olan tüm bağlantı noktalarının yerini doğru şekilde alır.

Dile ve ülkeye göre radyo araması
• Dil ve Ülke filtreleri artık Radyo Tarayıcı kataloğundaki mevcut tüm girişlerle güncellendi ve artık sabit bir listeyle sınırlı değil.
• Dil adları artık Radyo Tarayıcı bunları başka bir alfabeyle, kendi yerel biçiminde, kısaltmalar veya birden çok dilin birleşimi olarak sağladığında bile tanınıyor ve geçerli arayüz diline çevrilmiş olarak gösteriliyorlar. Sayılar, müzik türleri, ülkeler veya genel ifadeler gibi gerçek dilleri temsil etmeyen değerler hariç tutulmuştur.
• Güncelleme arka planda gerçekleşir ve Radyo Tarayıcıya ulaşılamadığında bile kullanılabilecek bir yedek liste tutulur.
• Çeviriden sonra aynı olan Radyo Tarayıcı girişleri artık tek bir açılır kutu öğesinde birleştirildi ve ekran okuyucularla sessiz geçişlerin önüne geçildi.

Ana iyileştirme: Konuşma okuma ve imleç arasındaki senkronizasyon
• Desteklenen tüm metinden konuşmaya motorları için konuşmayı okuma ve imleç hareketi arasındaki senkronizasyon önemli ölçüde iyileştirildi.
• “Okurken imleci hareket ettir” ayarı etkinleştirildiğinde, Sonarpad artık Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 ve OneCore için ortak bir ilerleme sistemi kullanıyor.
• İmleç, cümlelere ve cümle bölümlerine göre daha tutarlı bir dökümle gerçekte konuşulan metni daha kesin bir şekilde takip eder.
• İmlecin ilerlemeleri, gecikmeleri ve düzensiz atlamalarının yanı sıra bir konuşma motoru ile diğeri arasındaki davranış farklılıkları da önemli ölçüde azaltıldı.
• Duraklamalar, okumaya devam etme, belge arama ve konuşma motoru değişikliklerinden sonra doğru pozisyonun daha iyi korunması.

İki ayrı kanalda podcast kaydı
• “Mikrofonu ve sistem veya uygulama sesini iki ayrı dosyaya kaydet” seçeneği eklendi.
• Mikrofon ve başka bir kaynak birlikte kaydedildiğinde, Sonarpad artık yalnızca mikrofonu içeren bir dosya ve sistemden, tek bir uygulamadan veya seçilen uygulamalardan gelen sesi içeren ikinci bir dosya oluşturabilir.
• Kaynak ayırma hem MP3 hem de WAV'da mevcuttur.
• Seçenek etkinleştirilmezse Sonarpad normal olarak karışık kaynaklarla tek bir dosya üretmeye devam eder.
• Ayrı parçalar ses seviyelerini ayarlamayı, gürültüyü gidermeyi, tek bir kaynağı düzeltmeyi ve daha sonra podcast'leri, röportajları ve eğitimleri düzenlemeyi kolaylaştırır.

Canlı TV
• Canlı televizyona ayrılmış yeni bir bölüm eklendi.
• Kategorilere göre düzenlenmiş ulusal ve bölgesel kanallara göz atabilir, hızlı bir şekilde kanal arayabilir ve favorilerinizi yönetebilirsiniz.
• Şu anda yayında olan program hakkında bilgi eklendi ve TV rehberine danışıldı.
• Kanallar video penceresiyle veya video penceresi olmadan oynatılabilir.
• Bir yayını kaydedebilir veya kanalı dinlemeye devam ederken kaydedebilirsiniz.
• TV kayıt listesine doğrudan erişim eklendi.
• Bitmiş önceki programların görüntülenmesinden kaçınarak, geçerli saat, TV rehberi ve "Şimdi yayında" olarak gösterilen program arasındaki yazışma iyileştirildi.
• Video kapalıyken bile kanalın oynatılması daha güvenilir hale getirildi.

Planlanmış kayıtlar
• TV ve radyo kayıtlarını önceden planlama yeteneği eklendi.
• Her kayıt için kanal veya istasyonu, günü, başlangıç saatini ve dakikasını ve süresini seçmek mümkündür.
• 1 ila 1.440 dakika arasında özel bir süre mevcuttur.
• Kayıt bir kez, her gün veya haftada bir yapılabilir.
• Kayıtlar bölümü devam eden kayıtları, planlananları, beklenen tarih ve saati, süreyi ve başlamadan önce kalan süreyi daha net bir şekilde gösterir.
• Zamanlanmış kayıtlar Windows Görev Zamanlayıcı kullanılarak yönetilebilir, böylece Sonarpad açık olmadığında bile otomatik olarak başlamalarına olanak sağlanır.

Takvim
• Klavyeyle erişilebilen eksiksiz bir takvim eklendi.
• Önceki ve sonraki günlere bakabilir, hızlıca bugünün tarihine dönebilir, tatiller ve yıldönümleri hakkında bilgi edinebilirsiniz.
• Okunabilecek, dinlenebilecek veya kopyalanabilecek günün azizi ve günün alıntısı eklendi.
• Hatırlatıcıları ekleyebilir, düzenleyebilir ve silebilirsiniz.
• Hatırlatmalar tam zamanında veya önceden bildirilebilir, geçici olarak ertelenebilir ve tamamlandı olarak işaretlenebilir.
• Zaman damgalı hatırlatıcılar Windows zamanlamasını kullanabilir, böylece Sonarpad açık olmadığında bile görünürler.

Hava Durumu
• Hava durumu tahminlerine ayrılmış yeni bir bölüm eklendi.
• Bir şehri arayabilir ve yakın zamanda görüntülenen yerleri hızlı bir şekilde geri çağırabilirsiniz.
• Mevcut durum, sıcaklık, minimum ve maksimum değerler, nem, yağış olasılığı ve önümüzdeki günlere ilişkin tahminler mevcuttur.
• Celsius, Fahrenheit veya otomatik seçim arasında seçim yapabilirsiniz.

Sinemadaki filmler
• Şu anda sinemalarda olan ve yakında vizyona girecek filmlere danışmak için bir bölüm eklendi.
• Başlığa, olay örgüsüne, yayın tarihine ve fragman oynatmaya göre arama yapılabilir.

Google metinden konuşmaya
• Belgeleri okumak ve sesli kitaplar oluşturmak için kullanılabilecek yeni bir Google konuşma sentezi motoru entegre edildi.
• Mevcut sesleri görüntülemek, dile göre filtrelemek, indirmek ve artık ihtiyaç duyulmayanları silmek için bir ses yöneticisi eklendi.
• Hızı, ses seviyesini ve perdeyi kontrol edebilirsiniz.
• Google Natural seslerinde ton doğrudan motor tarafından uygulanarak daha doğal ve istikrarlı bir sonuç elde edilir.
• Maksimum sentez sürelerinin ses hızına uyarlanmasıyla Google TTS'nin yanıt verme yeteneği ve güvenilirliği iyileştirildi.
• Motor tepkisiz hale geldiğinde gereksiz bekleme süreleri azaltıldı ve hata ve kesintilerin daha iyi ele alınması.
• Eşzamanlı işlemler sırasında tanılama günlüğü yazımı daha kararlı hale getirildi.

EPUB belgeleri dizini
• Sonarpad artık EPUB kitaplarına gömülü dizini tanıyor.
• Dizinin varlığı duyurulur ve Görünüm menüsünden açabilirsiniz.
• Bölümler ve alt bölümler hiyerarşik olarak görüntülenir.
• Enter tuşuna bastığınızda kitapta seçilen noktaya hemen ulaşırsınız.

RSS haberleri ve kaynakları
• Haberler bölümü yeni arama ve organizasyon olanaklarıyla genişletildi.
• Haber dili seçeneği eklendi.
• RSS kaynakları içerisinde arama yaparak şehrinizin haberlerine göz atmanız mümkündür.
• Sonarpad topluluğu tarafından paylaşılan kaynakları keşfetme, bunları koleksiyonunuza ekleme ve topluluğa yeni RSS kaynakları gönderme yeteneği eklendi.

Podcast kaydı
• Yalnızca mikrofonu, tüm sistem sesini, tek bir uygulamayı, seçilen birden fazla uygulamayı veya mikrofonu ve uygulamaları aynı anda kaydedebilirsiniz.
• Mikrofon cihazını ve ses kaynağını seçebilir, ses düzeylerini ayrı ayrı ayarlayabilir ve kaynakların düzeyini gerçek zamanlı olarak kontrol edebilirsiniz.
• Kaydı duraklatma ve devam ettirme, MP3 ve WAV arasında seçim yapma, MP3 bit hızı seçimi ve kaydetme klasörü eklendi.
• Kayıt sırasında bilgisayarınızı aktif tutabilirsiniz.
• Ayrı dosyalar otomatik olarak farklı adlar alır ve mikrofon izinin sistem veya uygulamalardan anında tanınabilmesini sağlar.

Radyo
• Radyo bölümü kapsamlı bir şekilde yeniden düzenlendi.
• İstasyonları ada veya serbest metin, dil, ülke, şehir ve müzik türü veya kategorisine göre arayabilirsiniz.
• Favorilerin iyileştirilmiş yönetimi ve tüm filtreleri hızlı bir şekilde temizleme yeteneği eklendi.
• Sonarpad topluluğuna bir istasyon ekleyebilirsiniz.
• Canlı kayıt, "Kaydet ve oynat" modu, kayıtların listesi ve bunların silinmesi ve yönetimi eklendi.
• TV ve radyo kayıtları genel kayıtlar klasöründe ayrı klasörlerde saklanır.

Multimedya oynatma
• Multimedya oynatıcının kararlılığı önemli ölçüde iyileştirildi.
• Mpv'yi engelleyebilecek bir sorun düzeltildi ve oynatıcıyla iletişim daha güvenilir hale getirildi.
• Farklı türdeki multimedya dosyalarının açılması iyileştirildi.
• Sonarpad artık oynatma sırasında kullanılan ses seviyesini hatırlıyor.
• Akışların ve kayıtların iyileştirilmiş yönetimi ve oynatma sonrasında TV ve radyo pencerelerine sabit dönüş.
• Sonarpad'e doğrudan Windows'tan gönderilen dosyaların çift tıklatılması veya “Birlikte aç” gibi yöntemlerle açılması düzeltildi.
• MediaKit akışlarının hem ses modunda hem de aktif videoda iyileştirilmiş yönetimi.

PDF belgeleri
• PDF formlarında bulunan alanların tanınması eklendi.
• Sonarpad doldurulabilir alanları tanımlayabilir, bunları erişilebilir metin biçiminde sunabilir, değişiklik yapılmasına izin verebilir ve girilen verileri PDF olarak kaydedebilir.
• Özellikle çok baytlı karakterlere veya karmaşık yapılara sahip belgelerde, konuşma okuma sırasında imleç konumunun hesaplanması düzeltildi.
• Yeni ortak senkronizasyon sistemi, tüm konuşma motorlarında imleç hareketini daha da geliştirir.

Erişilebilirlik ve klavye kontrolleri
• Çeşitli program pencerelerindeki normal düzenleme komutlarının işleyişi iyileştirildi.
• Kopyalama, kesme, yapıştırma, tümünü seçme, geri alma ve yineleme artık alt ve iletişim pencerelerinde bile odağın olduğu alana doğru şekilde gönderiliyor.
• Braille ekranının düzgün şekilde güncellenmesini engelleyebilecek bir sorun düzeltildi.
• İkincil pencerelerde iyileştirilmiş odak yönetimi.
• Wikipedia penceresindeki dil seçimi düzeltildi.
• Araçlar menüsü işlevlerini kategoriye göre gruplandırma yeteneği eklendi.
• Sinemada Takvim, Hava Durumu ve Filmleri hızla açmak için yapılandırılabilir eylemler eklendi.
• Güncellemenin sonunda değişiklik günlüğünün iyileştirilmiş görüntüsü.

Sesli kitaplar
• Diyaloglar veya diğer kalıcı pencereler açıkken sesli kitap oluşturma işleminin iyileştirilmiş yönetimi.
• İlerleme kontrolü daha sağlamdır ve artık geçerli olmayan ses güncellemelerini göz ardı ederek donmaları, hatalı bildirimleri ve yanıt vermeyen pencereleri azaltır.
• Google TTS, hız, ses seviyesi ve perde kontrolüyle sesli kitap oluşturmak için de kullanılabilir.

Yapay zeka
• Varsayılan olarak kullanılan Gemini modeli 'gemini-3,5-flash' olarak güncellendi.

Genel düzeltmeler
• MPV ile oynatılırken meydana gelen birkaç çökme düzeltildi.
• Bazı ses ve video dosyalarının açılması düzeltildi.
• Oyuncuya gönderilen komutların yönetimi iyileştirildi.
• Sesli okuma sırasında imlecin sıfırlanması düzeltildi.
• Yardımcı pencerelerin metin alanlarındaki kısayolların işleyişi düzeltildi.
• Sesli kitap oluşturmanın kararlılığı iyileştirildi.
• Windows üzerinden harici olarak talep edilen dosyaların açılması düzeltildi.
• TV ve radyo kayıtlarının iyileştirilmiş yönetimi.
• Medya, RSS, TV ve EPUB'un genel yönetimi iyileştirildi.

Sürüm 0.7.1 – 2026-05-13

Haberler ve iyileştirmeler
• En son haberleri takip etmek, programın en son sürümünü indirmek, ziyaretçilerin yorumlarını okumak ve gelecekte tüm Sonarpad podcast'lerini dinlemek için yeni bir referans noktası olan resmi sonarpad.com web sitesi oluşturuldu. Resmi siteyi hızlı bir şekilde açmak için Yardım menüsüne “sonarpad.com'u ziyaret edin” öğesi de eklendi.
• Ses transkripsiyonu başlatıldığında aksanlı veya özel karakterli dosyaların başarısız olmasına neden olan sorun düzeltildi.
• Artık Görünüm menüsünde, Sözcük Kaydırma ve Kayıttan Yürütme Sırasında Videoyu Göster gibi öğeler, açık veya kapalı olarak her zaman doğru durumda görünecektir.
• Esc tuşuyla önceki sayfaya veya ekrana dönmenizi sağlayan iyileştirilmiş YouTube araması.
• Bir videonun oynatılabilir olup olmadığını kontrol etmek için bir ön kontrol eklendi. Oynatma da iyileştirildi: Sonarpad artık daha önce oynatılmamış, karma olarak işaretlenmiş videoları veya oynatma listelerini de oynatabiliyor.
• Otomatik yer imlerinin iyileştirilmiş yönetimi. Daha önce, Otomatik Yer İmleri açılıp kapatıldığında bu yer imleri kalıyordu; artık program, seçenek yeniden etkinleştirilene kadar bunları doğru bir şekilde yok sayar. Ayrıca bir medya dosyasının sonuna ulaştığınızda yer imi otomatik olarak silinir.
• Seçenek etkinken otomatik yer iminin RaiPlaySound'a kaydedilmemesi sorunu düzeltildi.
• RaiPlaySound'da, bağlamsal menüye podcast'i favoriler arasına doğrudan podcast kütüphanesine ekleme olanağı eklendi. Bu şekilde yeni bölümlerden de haberdar olursunuz. İlgili bölüme gitmeniz ve Uygulamalar butonuna basmanız gerekiyor: Podcast'lere ekle öğesini bulacaksınız.
• RaiPlay'den bir MP3 indirirken, mümkünse ses ile açıklanan parça tercih edilir.
• Artık BdCiechi kütüphanesine erişerek ve bir kitap indirerek, içinde bulunulan ayda kaç indirmenin kaldığı konusunda her zaman güncel bilgi sahibi olacaksınız.
• Aktif diyaloglarla iyileştirilmiş etiket yönetimi. Sonarpad artık her iki işlevi de doğru şekilde yerine getiriyor ve diyalog seçeneği etkin olsa bile etiket eklemenize olanak tanıyor.
• İyileştirilmiş ses ayarları, her motoru açıkça ayırarak ayarlamanın daha hassas olmasını sağlar. Ses profilleri her bir motor için ayarları doğru şekilde korur: Edge, Sapi5 ve Sapi4.
• Doğrudan seçeneklerden veya düzenleyicide Sekme tuşuna basılarak sesler panelinden dinlenme eklemek için bir etiket eklendi. Seçenekler şunlardır: 250 ms, 500 ms, 1 saniye, 2 saniye veya özel süre.
• YouTube'dan video oynatırken ve transkripsiyonu başlatırken oluşan davranış düzeltildi. Artık Alt+Tab tuşlarına basıldığında odak, doğru şekilde devam eden transkripsiyonun İptal düğmesi üzerinde olacaktır.
• Transkripsiyonlar artık sürecin sonunda otomatik olarak kaydediliyor.
• Wikipedia'dan içe aktarma iyileştirildi. Yalnızca bir bölümü okumayı seçebilir ve ardından makaleden Esc tuşuna basarak aramaya geri dönebilir veya makalenin tamamını içe aktarabilirsiniz. Ayrıca danışmak için Wikipedia'nın dilini de seçebilirsiniz.
• Ülkeye, dile ve türe göre radyo arayabileceğiniz, dünyanın her yerinden radyoların bulunduğu bir bölüm eklendi. Ayrıca Sonarpad veritabanına yerel radyolar da ekleyebilirsiniz, böylece diğer kullanıcılar da bunları dinleyebilir. Ayrıca favorilerinize bir radyo da ekleyebilirsiniz.
• Şu araçları seçerek rotaları hesaplamak için yol rotalarına ilişkin bir bölüm eklendi: yürüyerek, bisikletle, arabayla ve tekerlekli sandalyeyle. En kısa veya en hızlı rotanın mı hesaplanacağını ve geçilen belediyelerin gösterilip gösterilmeyeceğini seçebilirsiniz. Rota içe aktarıldıktan sonra görsel haritayı Dosya, Görüntüyü Kaydet menüsünden de kaydedebilirsiniz.
• Dosya menüsüne Yazdır öğesi eklendi. Sonarpad, programın kendisini kullanarak TXT dosyalarını yazdıracak ve orijinal düzeni mümkün olduğunca korumak amacıyla DOCX, PDF ve benzeri diğer dosyalar için ilişkili programı kullanacaktır.
• Her belge için, editörün içerik menüsünden erişilebilen bir çeviri hizmeti Sonarpad'e entegre edilmiştir. Kullanıcı, herhangi bir API anahtarı girmeden ücretsiz DeepL ve Google Translate hizmetlerinden yararlanabilecek; bunun yerine Gemini API anahtarı ekleyerek Gemini'yi kullanarak çeviri yapabileceksiniz.
• Çeviri menüsünde kullanıcı hedef dili seçebilir. Menü otomatik olarak yeniden sıralanır: Kullanıcı önce İngilizce'yi, ardından Fransızca'yı ve ardından İtalyanca'yı seçerse, bu üç seçenek dil menüsünün üst kısmında gösterilecektir.
• Kullanıcı Gemini API anahtarını girerse, herhangi bir makaleyi özetlemek için her zaman bağlamsal menüde bulunan Metni Özetle işlevine de erişebilecektir.
• Oynat menüsüne, bir medya dosyası oynatılırken görülebilen, geçerli medyayı bölmek için bir menü eklendi. Parça sayısına veya her parçanın süresine göre bölerek MP3, MP4 ve diğer formatlarla çalışır.

Sürüm 0.7.0 – 2026-04-25

Haberler
• Akış oynatma için mpv oynatıcıya destek eklendi. YouTube'daki ve desteklenen sitelerdeki videolar artık anında oynatılıyor; kullanıcı bunları saklamayı seçerse, daha önce olduğu gibi indirilir. Bir akış içeriğini metne dönüştürmeye başlarsanız, önce içerik indirilir ve ardından metne dönüştürülür. Mpv oynatıcı artık yerel videoları açmak ve altyazıları yönetmek için de kullanılıyor ve daha önce iyi yönetilemeyen çok sayıda formatla daha iyi uyumluluk sağlıyor.
• Sistem sesinin iyileştirilmiş podcast kaydı: Artık tüm sistem sesini mi, tek bir uygulamayı mı yoksa aynı anda birden fazla uygulamayı mı kaydedeceğinizi seçebilirsiniz. Bu seçenek normal kayıtla entegre olduğundan mikrofonu ayrı ayrı açıp kapatabilirsiniz.
• Hintçe dili eklendi. Çevrilmiş arayüz, RSS, değişiklik günlüğü ve Sonarpad yardımı eklendi.
• Editör sekmesine, yukarı ve aşağı okları kullanarak imleci her zaman satırın başına taşıma seçeneği eklendi.
• "Sesi Dönüştür" menü öğesine sesi M4B'ye dönüştürme seçeneği eklendi.

Düzeltmeler
• RSS kaynağı güncellenerek Corriere della Sera'daki makalelerin okunması düzeltildi. Artık makaleler her zaman yeniden güncellenecektir.
• Artık metin okurken bir sonraki favori öğeye geçiş yapan 'F10' tuşu düzeltildi.
• Bir podcast kaydı devam ederken, başka bir belgenin kapatılması artık etkin kaydı kapatmaz.
• "Akıştan ses çal..." seçeneğinden açılan YouTube yorumlarında, Sonarpad artık başlangıçta yalnızca en iyi 50 yorumu yüklüyor, her zaman bu yorumlardan gelen tüm yanıtları da içeriyor ve isteğe bağlı olarak tüm yorumları yüklemek için en alta bir giriş ekliyor.
• Yer işaretleri artık oluşturuldukları sırayı takip etmek yerine hem metin belgelerinde hem de medya dosyalarında konum sırasına göre gösteriliyor ve yönetiliyor. Bir yer imi aynı konumda zaten mevcutsa bir daha asla eklenmez.
• "Tüm sesli açıklamalar"da iyileştirilmiş yönetim: filmler artık tüm sesli tanımlamalı filmleri toplayan, Tab ile erişilebilen bir "Film" düğmesi aracılığıyla kurgudan ayrılıyor. Arama artık hem kurgu hem de filmlerde sonuç buluyor.
• Beyaz Sayfalar'a, geçerli sayfanın tüm sonuçlarını keşfetmek için Sekme ile erişilebilen bir düğme eklendi.
• RaiPlay'de, kaydetme sırasında, sesle açıklanan parçayı kullanarak MP4 formatında kaydetmeyi zorunlu kılan yeni bir 'Sesli açıklamalı MP4' seçeneği mevcuttur.
• Yer İmleri menüsüne, etkinleştirildiğinde yer imlerinin otomatik olarak yönetilmesine olanak tanıyan bir seçenek eklendi. Yerel veya akışlı bir dosyayı oynatıp kapattığınızda Sonarpad, ulaştığınız konuma göre otomatik olarak bir yer imi ayarlar ve dosyayı yeniden açtığınızda o noktadan devam eder. Aynı şey metin dosyaları için de geçerlidir: Bir metni açıp imleci hareket ettirirseniz, Sonarpad onu kapattığınızda o konumu hatırlayacaktır; bunun yerine okumaya başlarsanız, okunan son cümle kaydedilecek ve okuma tam olarak oradan yeniden başlayacaktır.
• Yerel veya akışlı dosyalar için video oluşturmayı gösterecek şekilde Görünüm menüsüne öğe eklendi. Video içeriği büyütülmüş bir pencerede görüntülenir; burada Alt tuşuna basmanız veya fareyi pencerenin üstüne getirmeniz dışında tüm kontroller gizlenir. Bu sayede görme engelli kullanıcıların daha büyük ve kullanışlı içeriklere sahip olması gerekmektedir.

Sürüm 0.6.9 – 2026-04-08

Düzeltmeler
• Dosyalarda Bul deneyimi iyileştirildi: Klasöre Gözat'ı açtığınızda odak hemen liste görünümüne getirilir; Enter ile bir sonuç açılarak tüm klavye komutları çalışmaya devam eder; Esc tuşuna basıldığında önceden seçilen sonuca geri dönülür; ve Alt+Tab tuşlarına basıldığında odak, arama alanına veya açıksa sonuçlara getirilir.
• F5 her zaman baştan okumaya başlar. Bu sorun artık düzeltildi ve okuma imlecin bulunduğu yerden başlıyor; önceki veya sonraki cümleye gitmek için 'Shift+F5' ve 'Ctrl+F5' de korunuyor.
• Hatta Git'e gittikten sonra Esc tuşuna basıldığında Sonarpad'den çıkılır. Odaklanma artık düzenleyicide doğru şekilde geri dönüyor.
• 'Kelime Kaydırma' seçeneği artık açık olan belgelere bile dosyayı yeniden açmaya gerek kalmadan hemen uygulanıyor.

Sürüm 0.6.8 – 2026-04-07

Haberler
• Whisper ile herhangi bir ses veya video dosyasını yazıya dökmek için Oynat menüsündeki yeni öğe. Model seçimi, isteğe bağlı CUDA desteği (NVIDIA video kartları), orijinal dili koruma ve zaman damgalarını değiştirme seçeneğiyle birlikte Seçenekler'de yeni bir "Yapay Zeka ve transkripsiyon" bölümü mevcuttur.
• Oynat menüsüne, açık medya klasöründe bulunan tüm desteklenen ses dosyalarını yazıya döken ve bunları özel bir ilerleme penceresi, mevcut dosyanın göstergesi ve iptal etme olanağı ile tek bir belgede birleştiren yeni "Geçerli klasörü kopyala" eyleminin eklenmesi. Ayrıca 'Alt+Shift+C' kısayoluyla da çağrılabilir.
• Sesli dikteyi, ses transkripsiyonuyla aynı şekilde çevrimdışı kullanma yeteneği eklendi. Varsayılan olarak dikteyi başlatmak için 'Ctrl+Shift+Boşluk' tuşlarına basarsınız ve dikteyi bitirmek için aynı kısayola basarsınız; kısayol tuşu Seçenekler'de özelleştirilebilir. İkinci aktivasyondan itibaren dikte daha hızlıdır çünkü motor hafızada hazır kalır; 4 GB'tan az RAM'e sahip bilgisayarlarda bu ön yükleme ve yeniden kullanım otomatik olarak devre dışı bırakılır.
• Editör Seçenekleri'ne, varsayılan olarak devre dışı bırakılan ve editör penceresini 'Esc' ile kapatan yeni bir ayar eklendi.
• Tüm RaiPlay videolarını görüntülemek ve yönetmek için, canlı yayınlar, vurgulanan içerik ve kataloğun tamamında arama dahil olmak üzere tüm içeriğin entegre yönetimiyle yeni bir bölüm eklendi.
• GR ve tiyatro da dahil olmak üzere mevcut tüm podcast'lerin katalog keşfi, küresel içerik araması ve oynatılmasıyla RaiPlay Sound yönetimi eklendi.
• Beyaz Sayfalar ve Sarı Sayfalar'daki tüm isimlerin aranması için isim, şehir ve adres girme imkanı (isteğe bağlı) ile yeni bir bölüm eklendi.
• Podcast araması artık varsayılan olarak "iTunes + Spreaker"ı kullanıyor ve her iki platformda da aynı podcast mevcut olduğunda yinelenen sonuçları filtreliyor.
• Geliştirilmiş Apple Podcast araması ve göz atma: Podcast araması, kategori gezinmesi ve kategoriye göre en popüler podcast'ler artık podcast dizini için seçilen ülkeyi kullanıyor. Seçenekler > RSS / Podcast'te sistem ülkesini kullanmak için 'Otomatik'ten çıkabilir veya başka bir ülkeyi manuel olarak seçebilirsiniz.
• Apple podcast kategorileri için sonuç sınırı artırıldı. İlk açıldığında her zaman olduğu gibi ilk 50 sonuç yüklenir; 'Daha fazla sonuç yükle'yi seçerseniz, Sonarpad toplam 200'e kadar sonuç yükler (Apple tarafından uygulanan sınır) ve daha akıcı bir deneyim sürdürürken sonraki sayfalara gitmenize olanak tanır.
• Kısmi özelliklerle de olsa Sonarpad Mac'te de mevcuttur. Proje bağlantısı: https://github.com/Ambro86/Sonarpad-Mac

İyileştirmeler
• Podcast dizinine 50'den fazla seçilebilir ülke eklendi, böylece çok daha fazla ülke kataloğundan seçim yapabilirsiniz.
• "Akıştan ses çal..." artık herhangi bir metin dizesi yazarak YouTube'da arama yapmanıza veya sonuçları görüntülemek için bir YouTube kanalının veya oynatma listesinin bağlantısını yapıştırmanıza da olanak tanıyor.
• "Akıştan ses çal..." bölümünde iyileştirilmiş sonuçlar gösteriliyor: YouTube girişleri artık başlık, süre, kanal ve görüntülemeleri daha net bir biçimde içeriyor.
• "Akıştan ses çal..." artık YouTube yorumlarını da destekliyor: Sağ Ok tuşunu kullanarak bunları içerik menüsünden açabilir, yanıtları okuyabilir ve yorum dizilerini genişletebilirsiniz.
• "Akıştan ses çal..." seçeneğine YouTube kanallarını ve oynatma listelerini favorilere kaydetme olanağı eklendi: içeriksel menü yoluyla sonuçlardan eklenebilirler, YouTube URL'si/sorgu alanının hemen ardından Sekme ile erişilebilen Favoriler listesinden doğrudan açılabilirler ve içeriksel menü aracılığıyla her zaman aynı listeden kaldırılabilirler. YouTube arama sonuçlarında içerik menüsü yalnızca kanallar ve oynatma listeleri için kullanılabilir.
• Artık "Akıştan ses çal..." seçeneğinde, bir site erişim istediğinde Sonarpad kimlik bilgilerini isteyebilir. Kullanıcı bunları girebilir, site için kaydedebilir ve daha sonra kaydedilen kimlik bilgilerini Seçenekler > Ses'ten yönetebilir.
• "Akıştan ses çal..." sırasında odaklanma iyileştirildi, böylece indirme ve dönüştürme sırasında ilerleme penceresi daha sabit kalıyor.
• Ses menüsüne iki yeni okuma eylemi eklendi: Metin okurken atlamak için özelleştirilebilir kısayollarla birlikte 'Önceki cümle' ve 'Sonraki cümle'.
• 'Dosyayı yorumlayıcıyla çalıştır' için varsayılan kısayol artık 'Ctrl+Shift+F5' olmuştur, dolayısıyla 'Shift+F5' varsayılan olarak 'Önceki cümle' için kullanılabilir.
• Program artık çok sayıda değişiklik içeren büyük dosyalarda Tümünü Toplu Değiştirme işlemini gerçekleştirebilir.
• Araçlar'dan veya Alt+Shift+B kısayoluyla erişilebilen BdCiechi kitaplığı için tam destek eklendi. Kitap aramak, indirmek, en son haberleri okumak, örnek metin indirmek mümkündür. Bir dosya kaydedildiğinde, onu doğrudan Sonarpad'de açabilir ve programda bulunan yüksek kaliteli seslerle okuyabilirsiniz.
• Giuliano artigianato ile yapılan işbirliği sayesinde BdCiechi kütüphanesinde çeşitli iyileştirmeler yapıldı: kullanıcı adı ve şifre Sonarpad'de 30 gün boyunca saklanabilir, sonrasında tekrar oturum açmak gerekir; ancak kullanıcı verileri saklamamayı seçerse Sonarpad açık kaldığı sürece veriler yeniden yazmaya gerek kalmadan yeniden kullanılır. Eser araması, girilmeyen aksanlar düzeltilerek normalleştirildi, böylece örneğin `giosue` arandığında `Giosuè` de bulunur. Ayrıca BdCiechi yeniden açıldığında pencerenin doğru şekilde yeniden etkinleştirilmemesi sorunu da düzeltildi. BdCiechi kataloğu artık yalnızca ilk kullanımda indirilmekte ve daha sonra yalnızca gerektiğinde güncellenmektedir. Ayrıca kütüphaneden çıkış yapmak için bir düğme eklendi ve kaydedilen dosyanın adına yazar artık kitap başlığının önüne ekleniyor.
• RSS penceresi entegre bir makale ön izlemesiyle iyileştirildi, böylece metne doğrudan oradan başvurulabilir ve makalenin tamamını editörde açmadan önce Tab ile hızlı bir şekilde ulaşılabilir.
• Diğer öğeler mevcut olduğunda RSS'deki kaynakların altına açık bir "Diğer haberleri yükle" öğesi eklendi; Enter tuşuna basıldığında sonraki blok yüklenir ve odak ilk yeni haber öğesine taşınır.
• Rai sesli açıklamaları için destek eklendi. You can view the most recent audio descriptions and all audio descriptions sorted by category. To access the service you need to request a code from the author. The program will automatically propose sending the email. Sorun olması durumunda, e-postanın gövdesine adınızı ve soyadınızı yazarak 'Sonarpad kod isteği' konusuyla ambro86@gmail.com adresine göndermeniz yeterlidir. Bütün bunlar hizmetin kötüye kullanılmasını önlemek için gereklidir. Ayrıca ses açıklamalarının akış adresini panoya kopyalamak için içerik menüsü de eklendi.
• Seçenekler > Ses'te ses profili yönetiminin eklenmesi: profil eklemek, yeniden adlandırmak ve silmek mümkündür.
• Konuşma sözlüğünde, bir değişikliği eklerken veya değiştirirken, artık büyük/küçük harfe saygı göstererek veya yok sayarak değişikliğin uygulanıp uygulanmayacağını seçmenize olanak tanıyan “Hassas harf” kutusu mevcuttur.
• Seçenekler > Ses bölümünde oynatma sırasında geri sarma aralığı seçenekleri, 1 saniyeden 2 saate kadar yeni değerlerle genişletildi.
• Dmitriy sayesinde Rusça çeviri eklendi.
• Added in Options > Audio a new choice for the name format of audiobook parts: `Title + number`, `Number only` or `Number + title`.
• RSS makalelerinin içerik menüsüne makaleyi favorilere ekleme seçeneği eklendi.
• "Favoriler" RSS kaynağı tanıtıldı: silinebilir ve bir sonraki favorilerinize bir makale eklediğinizde otomatik olarak yeniden oluşturulur.
• Kaynakları yukarı/aşağı taşımak için RSS klavye kısayolları eklendi: 'Ctrl+Shift+Yukarı Ok' ve 'Ctrl+Shift+Aşağı Ok'.
Düzeltmeler
• "Play audio from streaming..." and audio description playback now respect the cache limit already set for podcasts.
• Corrected the import from Wikipedia, which in some pages did not correctly report the citations present in the text.
• Web sayfası ayrıştırıcısı iyileştirildi: Bazı WordPress sayfalarında liste öğeleri ve bazı bölüm başlıkları eklenmiyordu.
• Artık "Satıra Git" kullanılarak alan, geçerli satırla önceden doldurulur.
• Artık iTunes tarafından kabul edilen dosyaları oluşturan podcast'lerin ve RSS'nin OPML dışa aktarımı düzeltildi.
• Added localized confirmation messages for successful OPML import and export of RSS feeds and podcasts.
• "Akıştan ses çal..." seçeneğinde bir arama dizesi yazıp sonuçlardan bir YouTube kanalı seçmenin, kanalın videolarını açmak yerine programın donmuş gibi görünmesine neden olabilmesi sorunu düzeltildi.
• Medya dosyalarının düzeltilmiş transkripsiyonu: Artık, oluşturulan belge Alt+F4 tuşlarıyla kapatıldığında Sonarpad, dosyayı kaydetmek isteyip istemediğinizi sorar ve metnin ilk satırı yerine, kopyalanan dosyanın adına göre doğru adı önerir.
• Açık dosyalar listesinin Pencere menüsü yerine Yardım menüsünde gösterilmesine neden olan bir hata düzeltildi.
• Akışta oynatmanın başlayabildiği ancak indirilen dosya zaten hedef formattayken "Akışını indir" penceresinin açık kaldığı uç durum düzeltildi.
• MP3 akışındaki dönüşüm davranışı düzeltildi: akış zaten MP3 olduğunda ve kullanıcı açık bir MP3 bit hızı seçtiğinde (örneğin 128 kbps), Sonarpad artık dönüşümü atlamak yerine seçilen bit hızına yeniden kodluyor.
• 'Alt+Shift+L' kısayolu düzeltildi: artık oynatma sırasında bölüm listesini doğru şekilde açıyor.
• 'Alt+Shift+T' kısayolu düzeltildi: Artık Araçlar menüsünü açmak yerine “Geçerli Sesi Metne Yaz” seçeneğini doğru şekilde başlatıyor.
• Oynat menüsündeki `.` düğmesinin davranışı düzeltildi: artık Durdur'a eşdeğerdir ve oynatıcıdan veya bölümden çıkmadan yalnızca geçerli şarkıyı durdurur.
• Son Dosyalardan açılan medya için Oynat menüsündeki kaydetme öğesi düzeltildi: dosya yerel bir Sonarpad önbelleğinden geldiğinde, dosyayı kaydetmeye yönelik yerelleştirilmiş seçenek artık bu durumda da doğru şekilde gösteriliyor.
• Zaten bir ses çalınıyorsa, transkripsiyona başladığınızda Sonarpad, başlamadan önce o sesi otomatik olarak duraklatır.
• Vikipedi'den bir makale içe aktarılırken içe aktarmanın başarılı olabilmesine rağmen makale metninin ekranda gösterilmemesi sorunu düzeltildi.
• Yerel medya dosyalarına gömülü podcast bölümleri için destek eklendi (örn. MP3 bölüm meta verileri): beslemeler/URL'ler bölüm sağlamadığında, Sonarpad bunları arka planda indirilen dosyadan okur, böylece oynatma hemen başlar ve bölümler mevcut olur olmaz uygulanır.
• Normal yerel medya dosyaları olarak indirilen ve açılan podcast bölümleri için okuma bölümleri düzeltildi: gömülü bölümler artık bu durumda da mevcut; yalnızca Podcast penceresinden oynatmayı başlatarak değil.
• MP3 sesli kitaplarının SAPI4 ve SAPI5 ile sonlandırılması düzeltildi: Son dosya artık doğru bir şekilde sonlandırılıyor ve uzun dışa aktarımlardan sonra eksik veya hassas dosyalar önleniyor.
• Tüm sesli kitap oluşturma modlarında sonlandırma aşaması için açık bir ilerleme çubuğu eklendi: Sonarpad, oluşturulduktan sonra bitişi duyurur ve görünür ilerlemeyle birlikte gösterir.
• Diyalog seslerindeki bir hata düzeltildi: birinci ve ikinci diyalog seslerinin hız/yükseklik/ses seviyesi parametreleri artık sentez sırasında doğru şekilde uygulanıyor.
• Japonca `.txt` dosyaları için geliştirilmiş kodlama algılaması: Mojibake durumlarına güvenli Shift_JIS/CP932 geri dönüşü eklendi ve UTF/aksan işaretleri/Çince'deki mevcut davranış korundu.
• Dahili güvenlik yeniden düzenlemesi: mümkün olan yerlerde güvenli uygulamalara geçiş ve güvenli olmayan kod satırlarının büyük ölçüde azaltılması.

Sürüm 0.6.7 – 2026-03-02
İyileştirmeler
• DJ Graco sayesinde Lehçe çeviri güncellendi.
• Litvanca çeviri eklendi.
• Çince çeviri eklendi.
• Artık projenin Sürümler bölümünde beta derlemeleri sık sık yayınlanacak, böylece kullanıcılar bir sonraki kararlı sürümden önce yeni değişiklikleri test edebilecek.
• Üç nokta karakteri eklemek için 'Ctrl+.' kısayolu eklendi (…).
• Podcast bölümleri için geliştirilmiş destek: bölümlerin MP3 dosyasına gömülü olmadığı doğrudan/akış bölümlerinde bile bölüm gezinme artık daha güvenilir; mevcut olduğunda geri dönüş olarak besleme/URL'deki bölüm meta verileri kullanılıyor. 'Ctrl+Alt+Page Up' (önceki bölüm) ve 'Ctrl+Alt+Page Down' (sonraki bölüm) kısayolları eklendi.
• 'Belgeler\\Sonarpad'deki çıktı klasörleri yeniden düzenlendi: dosyalar artık eski konumlardan otomatik geçişle özel 'sesli kitaplar', 'belgeler', 'kayıtlar' ve 'medya' alt klasörlerine kaydediliyor.
• Çok büyük metin dosyaları (hatta 60 MB) için geliştirilmiş destek: özellikle ekran okuyucularla daha sorunsuz açma ve satır satır gezinme.
• Bağış metni ve NSIS kurulum çevirileri (Basitleştirilmiş Çince ve Litvanca'da yeni yükleyici dizeleri ve ayrıca Ukraynaca kurulum çevirisinin tamamlanması) dahil olmak üzere tüm diller için güncellenmiş kılavuzlar ve güncellenmiş uygulama yerelleştirme kaynakları.
• Kayıt doğrulamayla birlikte çevrimiçi işlevler için genel ağ proxy desteği (HTTP/HTTPS ve SOCKS5/SOCKS5H) eklendi Seçenekler: Geçersiz proxy'ler rapor edilir ve otomatik olarak kaldırılır.
• Araçlar'a yeni bir işlev eklendi: Bir URL (YouTube veya doğrudan medya bağlantısı) girmenize, çıktı biçimini ve kalite/bit hızı profilini seçmenize (MP3 ve MP4 için orijinal kalite/bit hızı dahil) ve Sonarpad'in ses oynatıcısında oynatmayı başlatmanıza olanak tanıyan "Akışlı ses oynat...".
• Sisteme multimedya Oynat/Duraklat düğmesi (kulaklık/klavye) desteği eklendi: artık hem medya oynatmayı hem de metin okumayı duraklatmayı/devam ettirmeyi kontrol ediyor (her ikisi de aktif olduğunda medya oynatıcıya öncelik veriliyor).
• Son kullanılan belgelerin listesini hızla temizlemek için Dosya > Son Kullanılan Dosyalar menüsüne "Son Kullanılan Dosyaları Boşalt" adlı yeni bir öğe eklendi.
• Ses dönüştürme ve podcast kaydında genişletilmiş bit hızı seçenekleri: hizalanmış doğrulama ve kodlayıcı yönetimiyle daha düşük değerler eklendi (64/96 kbps) ve 320 kbps'ye kadar genişletilmiş MP3.
• 60 dakikaya kadar genişletilmiş sesli kitap bölme seçenekleri.
• Sesli kitabın bölümlere ayrılması iyileştirildi: artık bölüm sayısı 1'den 100'e kadar doğrulamayla manuel olarak girilebiliyor.
• Belgelerin tam okunmasını ve gezinmesini sürdürürken metinde yanlışlıkla yapılan değişiklikleri engellemek için yeni Görünüm > Salt Okunur modu eklendi.
• Program güncellemeleri sırasında erişilebilen bir ilerleme çubuğu eklendi, böylece ekran okuyucular indirme sürecini gerçek zamanlı olarak takip edebilir.
• NVDA'nın odağını bozmadan ana pencereye karakter sayısını, kelimeleri ve satır/sütun konumunu içeren yeni bir ayrı durum çubuğu eklendi (örnek: "Karakterler (boşluklu): 11. | Kelimeler: 2. | Ln 1, Sütun 12").
• Görünüm menüsüne, Seçenekler'i açmadan satır kaydırmayı hızla etkinleştirmek/devre dışı bırakmak için yeni Sözcük Kaydırma öğesi eklendi.
• Düzen > Metin menüsüne Ctrl+Shift+ kısayollarıyla girintiyi artırmak/azaltmak için yeni eylemler eklendi. (girinti) ve Ctrl+Shift+, (girintiyi azalt), çünkü “Girişleri düzenleyicide göster” etkinken Sekme tuşu girişler panelinde gezinmek için ayrılmıştır.
• Arayüz diline uyarlanmış formatla, RSS makaleleri ve podcast bölümleri için yerelleştirilmiş tarih ve saat gösterimi eklendi.
• Seçilen makaleyi e-postayla paylaşmak için RSS içerik menüsüne yeni bir öğe eklendi.
• Seçenekler > RSS ve Podcast'lere ayrıntılı silme onayı seçenekleri eklendi: RSS için (özet akışı/makale/her ikisi de/yok) ve Podcast'ler için (podcast/bölüm/her ikisi de/yok).
• Ctrl+C (Seçenekler > RSS ve podcast) ile yapılandırılabilen hızlı RSS kopyası eklendi: başlığı, URL'yi, makale içeriğini veya hepsini birlikte kopyalayın.
• Birleşik RSS akışı: "Kaynak Ekle" artık ayrı bir aramaya gerek kalmadan hem yayın URL'lerini hem de anahtar kelimeleri (Google Haberler akışının otomatik oluşturulmasıyla) kabul ediyor.
• Artık Ctrl+A'ya basmak, ekran okuyucularla daha net geri bildirim sağlamak için eylemin tamamlandığını duyuruyor.
• F3 "Sonraki Bul"a ek olarak, Düzen menüsüne "Öncekiyi Bul" için Shift+F3 kısayolu eklendi.
• Tekil/çoğulların doğru yönetimiyle değişiklik onay mesajı iyileştirildi (örneğin, "1 değişiklik" ve "2 değişiklik").
• Varsayılan Otomatik (arayüz dili) ve manuel geçersiz kılma olanağı ile Sözlük penceresine arama dili seçimi eklendi.
• Kısayol tuşlarını özelleştirmek için Seçenekler'e yeni bir Kısayollar sekmesi eklendi; çakışma tespiti ve bir kısayol zaten başka bir eyleme atandığında uyarı verildi.
• Komut satırı parametreleri için başlangıç desteği eklendi: `-h`/`--help` hızlı yardımı gösterir ve `--version` program sürümünü gösterir.
• Hız ve perdenin manuel olarak ayarlanması daha net hale getirildi: alanlar artık 100'ü merkeze alan bir ölçek kullanıyor; burada 100 normal değerdir.
• Hem Seçenekler > Ses'te hem de düzenleyicinin sesler panelinde Microsoft seslerinin seçimi iyileştirildi: sesleri dile göre filtrelemek için yerelleştirilmiş bir dil birleşik giriş kutusu eklendi, "yalnızca çok dilli sesler" modunu dile göre bölünmeyen tek bir liste olarak korudu (etkin olduğunda dil birleşimi gizlendi).
• Ana arayüzle aynı ses modelini kullanarak (sistem, Edge dil filtresi, etiketlerle ses ve hız/ses perdesi/ses seviyesi); ayrıca alternatif diyaloglara aynı kontrollere (sistem, Edge dil filtresi, ses, hız/ton/ses seviyesi) sahip ikinci isteğe bağlı diyalog sesini ekledi; diyalog kuralları, belgenin metnini değiştirmeden ".ini" konfigürasyonunda kaydedilir.
• Geri Al etiketi iyileştirildi: Düzenle > Geri Al öğesi artık geri alınacak eylemi (örneğin, metni düzenleme, yorum/yorum satırlarını kaldırma veya öğe etiketleri ekleme) gösteriyor ve geri alınacak bir şey olmadığında kullanılamıyor.
Hata düzeltmeleri
• RTF açma desteği düzeltildi: `.rtf` dosyaları artık ham RTF işaretlemesi olarak değil, okunabilir metin olarak çıkartılıyor ve görüntüleniyor (örn. `{\\rtf1...}`).
• Çince metin dosyalarının GB18030/GBK kodlamasında açılması düzeltildi: Sonarpad artık okunamayan metinlerden (mojibake) kaçınarak bunları doğru bir şekilde algılıyor ve kodunu çözüyor.
• Bölüm meta verileri ve bölüm işaretleyicileri ile M4B sesli kitaplarının oluşturulması iyileştirildi; oluşturulan M4B dosyalarındaki "sincap" sorunu (ses çok hızlı/tiz) düzeltildi.
• Sesli kitap kaydetme penceresindeki bit hızı arayüzü düzeltildi: İtalyanca sabit kodlu metinler kaldırıldı ve seçilebilir bit hızları arasına 64 kbps seçeneği eklendi.
• "Tümünü Kaydet" seçeneği düzeltildi (Ctrl+Shift+S): Değiştirilen tüm açık belgeler artık güvenilir bir şekilde algılanıyor (yeni/kaydedilmemiş sekmeler dahil) ve kaydetme her dosyada doğru bir şekilde devam ediyor ve gerektiğinde "Farklı Kaydet" seçeneği açılıyor.
• Google Haberler RSS makalelerinin sabit sıralaması: tarih mevcut olduğunda makaleler artık en yeniden en eskiye doğru gösteriliyor.
• Sözlük penceresindeki NVDA etiketi ilişkilendirmesi düzeltildi: arama alanı ve dil açılan kutusu artık doğru etiketi duyuruyor.
• RSS/Podcast Özellikleri penceresindeki klavye kullanımı düzeltildi: Sekme/Shift+Sekme, Tamam düğmesine ulaşır, Enter, Tamam'ı etkinleştirir, Esc güvenli bir şekilde kapanır ve odak, RSS/Podcast listesine doğru şekilde döner.
• RSS/Podcast'teki geri alma geçmişi düzeltildi: Ctrl+Z artık yalnızca son eylem için değil, kaldırma işlemleri (makaleler/bölümler ve kaynaklar) için de çok düzeyli geri almayı destekliyor.
• RSS/Podcast'te açık mesajlarla iyileştirilmiş kaldırma duyuruları (RSS kaldırıldı, RSS makalesi kaldırıldı, podcast bölümü kaldırıldı).
• RSS/Podcast'te silme/iptal etme sonrasında iyileştirilmiş odak yönetimi: RSS'de ilk besleme gerektiğinde güvenilir bir şekilde seçilir ve gecikmeli yeniden seçim sırasında ekran okuyucu reklamlarının tekrarı azaltılmıştır.

Sürüm 0.6.6 – 2026-02-13
İyileştirmeler
• Metni sesli okumaya hızlı bir şekilde hazırlamak için Düzenle menüsüne "TTS için Otomatik Biçim" eklendi (işaretlemeyi/alıntıları kaldırır ve kesikli satırları yeniden oluşturur).
• Öğe etiketlerinin eklenmesi iyileştirildi: Artık bir seçim varsa, etiketler hem tek bir satıra hem de seçilen birden çok satıra doğru şekilde uygulanıyor.
• Varsayılan sesli kitap kaydetme klasörünü seçmek için Ses ayarlarına bir seçenek eklendi (varsayılan: Documents\\Sonarpad Audiobooks).
• Sesli kitap kaydetme penceresinde, parçalara bölme etkinken, oluşturulan parçalara ayrılmış bir alt klasör oluşturmak için yeni bir seçenek eklenmiştir (varsayılan olarak etkindir).
• Sesli kitap dışa aktarımı artık MP3'leri Edge, SAPI5 ve SAPI4 sesleri için kullanıcı tarafından seçilen bit hızlarıyla stereo olarak kaydediyor.
• Köprü aracılığıyla 32 bit SAPI5 sesleri için destek eklendi, böylece yalnızca 32 bit motorlarda mevcut olan sesler de kullanılabilir.
• Özel bir "Ses ve ses" menüsündeki ses işlevleri yeniden düzenlendi ve desteklenen herhangi bir multimedya dosyasını MP3, AAC, OGG, Opus, FLAC, WAV ve AIFF'ye dönüştürmek için yararlı olan "Sesi dönüştür" öğesi eklendi/açıklandı.
• RSS/podcast kaynağının tamamını silmeden, son kaldırma işleminin (tek makale/bölüm veya podcast/RSS beslemesinin tamamı) iptal edilmesiyle, ayrı ayrı RSS makalelerinin ve ayrı podcast bölümlerinin (Del tuşu + onay ile içerik menüsü) kaldırılması eklendi.
• RSS penceresine RSS beslemelerinin OPML'ye aktarımı eklendi, böylece mevcut kaynaklar kolaylıkla kaydedilip yeniden içe aktarılabilir.
• RSS penceresine "RSS'yi anahtar kelimeye göre ara" işlevi eklendi: Bir anahtar kelime girildiğinde, Google Haberler RSS URL'si otomatik olarak oluşturulur ve önceden doldurulmuş kaynak ekleme penceresi açılır, böylece tematik yayınlar tek bir adımda oluşturulur.
• Mila Kuran sayesinde Sırpça çeviri eklendi.
• Ivan Shtefuriak sayesinde Ukraynaca çeviri eklendi.
• Medya dosyalarının birden fazla açılması eklendi: birden fazla dosyanın birlikte açılması, geçerli dosyayı değiştirmek yerine bir oynatma sırası oluşturur.
• Oynatma sırasında değişken arama kısayolları eklendi: 1 dakikalık tabanla, Sol/Sağ Ok 60 saniye hareket eder, Shift+Sol/Sağ Ok 20 saniye hareket eder ve Ctrl+Sol/Sağ Ok 3 dakika hareket eder.
• Oynatıcıdaki önceki/sonraki şarkı için kısayollar eklendi: Ctrl+Page Up ve Ctrl+Page Down.
• "Sesi sıfırla" öğesi eklendi ve sıfırlama işlemlerini "Hızı sıfırla" ve "Perdeyi sıfırla" ile birlikte Play'deki özel bir "Sıfırla" alt menüsüne gruplandırdı.
• Geliştirilmiş yükleyici: setup.exe artık desteklenen tüm dosya türlerini ilişkilendirme veya tek tek uzantıları manuel olarak seçme arasında seçim yapmanıza olanak tanıyor; MSI artık özellik ağacında uzantıya göre seçeneği de gösteriyor (varsayılan değişmedi: tümü etkin).
• Halihazırda açık olan dosyalardan birine hızla geçiş yapmak için "Belgeleri Aç..." öğesini içeren yeni "Pencere" menüsü eklendi.
• Görünüm > Yazı Tipi öğesi güncellendi: Artık seçicinin tamamı yerine, önceden ayarlanmış olan metin boyutunu koruyan ortak yazı tiplerini (Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman, Georgia) içeren hızlı bir alt menü var.
• RSS ve podcast'lerin iki farklı duyuruyla iyileştirilmiş okunması: kaynak düğümleri, feed/podcast'te güncellemeler olduğunda "yeni öğeleri" duyururken, bireysel RSS makaleleri ve bireysel podcast bölümleri "okunmadı"/"oynatılmadı" ifadesini duyurur; davranış Seçenekler'den devre dışı bırakılabilir.
Hata düzeltmeleri
• Satır içi HTML yorumları (<!-- ... -->) içeren kitaplar için EPUB metni ayrıştırma düzeltildi: bölüm metni artık kısmen veya tamamen atlanmak yerine doğru şekilde çıkartılıyor.
• İspanyolca Vikisözlük sözlüğü ve sözlük önbellek yönetimi düzeltildi: "agua" gibi kelimeler artık doğru şekilde bulunuyor ve eski "kelime bulunamadı" önbellekleri artık yeniden kullanılmıyor.
• Bazı İspanyol kaynakları (örneğin El Mundo) için RSS makalelerinin içe aktarımındaki kodlama düzeltildi: aksanlar ve "ñ" artık geçici düzenleyicide doğru şekilde korunuyor.
• Orta Avrupa dil dosyalarının (örn. Çekçe/Lehçe) ANSI kod çözme işlemi düzeltildi: Sonarpad artık UTF-8 ile ANSI'yi daha iyi ayırt ediyor ve bozuk aksan işaretlerini önleyerek doğru kod sayfasını (Windows-1250 dahil) seçiyor.
• URL'de parametreler bulunan RSS kaynaklarının kalıcılığı düzeltildi (örn. `rss.aspx?c=...`): bu yayınlar artık Sonarpad yeniden başlatıldıktan sonra doğru bir şekilde kaydediliyor ve geri yükleniyor.
• Google Drive işaretçi dosyalarının (`.gdoc`, `.gsheet`, `.gslides`) Dosya Gezgini içerik menüsünden açılması düzeltildi: Doğrudan okuma "Yanlış işlev (işletim sistemi hatası 1)" ile başarısız olursa, Sonarpad artık kabuk açık bir geri dönüş kullanıyor ve belge doğru şekilde açılıyor.
• Eski Excel `.xls` dosyalarının okunması düzeltildi (Excel 2010): eski ikili dosyalar artık bozuk metin göstermek yerine doğru şekilde tanınıyor/kodları çözülüyor (örn. `ÐÏ_à¡±...`).
• Yazım denetleyicisi duyuru akışı düzeltildi: hatalar artık metin yeniden okunduğunda yeniden duyuruluyor ve silinip yeniden yazıldığında aynı hata yeniden bildiriliyor.
• Satır düzeyinde düzeltilmiş metin işlemleri (örn. Ctrl+Q / Ctrl+Shift+Q, sırala/tersine çevir/tek satırları/birleştir): Shift+Aşağı Ok ile tek bir satırın seçilmesi artık bitişik satırları birleştirmez veya kesmez.
• Satır metni işlemlerinde çok satırlı seçimlerin işlenmesi düzeltildi (Ctrl+Q / Ctrl+Shift+Q ve ilgili araçlar): RichEdit yalnızca CR'ye yönelik satır ayırıcılar sağladığında, metin doğru şekilde normalleştirilir ve seçilen tüm satırlar, karakter kesmeleri olmadan işlenir.
• Çok dilli girişlerde paragraf tekrarlarına neden olabilecek görünür boşluk/sekme/yeni satır simgeleri (␠/U+2420, ␣/U+2423, ␉/U+2409, ␊/U+240A, ␍/U+240D, ␤/U+2424) için genişletilmiş TTS giriş normalleştirmesi.
• Benzersiz bir doğrulama hattıyla iyileştirilmiş Edge TTS metin temizleme: garip/görünmez boşlukların normalleştirilmesi, uzun noktalama işareti dizilerinin sıkıştırılması ("...", "!!!", "???" gibi) ve oynatma döngülerini önlemek için yalnızca noktalama işaretlerinden oluşan parçaların atlanması.
• MP3/podcast akışları için sabit oynatma süresi duyurusu (Ctrl+I): geçerli süre artık parça süresiyle sınırlıdır ve konum sonunu geçerse oynatma otomatik olarak durdurulur.
• Geliştirilmiş yükleyici yerelleştirme kapsamı: setup.exe artık Çekçe, Lehçe, Fransızca ve Sırpça'yı da içeriyor; MSI ise sürüm karışıklığını önlemek için tek bir ABD paketi olarak kalıyor.
• İçerik menüsü girişlerinin kaldırma temizliği düzeltildi: "Sonarpad ile aç" özelliği artık eski kayıt defteri senaryolarında bile güvenilir bir şekilde kaldırılıyor.
• SAPI5 ile duraklatma/devam ettirme güvenilirliği düzeltildi: F4 ile duraklatmak artık doğru şekilde çalışıyor ve devam etmek, baştan başlamak yerine istenen noktaya geri dönüyor.
• Medya oynatımında sabit duraklatma + arama + devam ettirme akışı: Duraklattıktan ve Sol/Sağ Ok ile hareket ettirdikten sonra, Space tuşuna basmak, oynatmayı durdurmak veya baştan başlatmak yerine güvenilir bir şekilde mevcut noktadan devam ettirir.

Sürüm 0.6.5 – 2026-02-07
İyileştirmeler
• Arturo Fernandez Rivas sayesinde geliştirilmiş İspanyolca çeviri.
• Varsayılan yayınlar güncellendi: Affaritaliani, HuffPost Italia, La Gazzetta dello Sport. Kablolu İtalya kaldırıldı.
• EPUB sesli kitaplarını bölümlere göre bölme seçeneği eklendi.
• Podcast kayıt penceresi artık bağımsızdır, böylece aynı anda kayıt yapabilir ve Sonarpad programını kullanabilirsiniz!
• RSS makaleleri artık özel bir geçici sekme (yerelleştirilmiş başlık) kullanıyor; Normal bir belgeye dönüştüğünde Kaydet ile.
• Ekran okuyucu mesajları artık mümkün olduğunda JAWS'a da gönderiliyor.
Hata düzeltmeleri
• İmleç okuma (F5) artık tam olarak imleç noktasından başlıyor. Önceden, imleç konumu CRLF/UTF-16 konumlarıyla eşleşmediğinden birkaç satır yukarıdan başlayabiliyordu.
• Yeniden çizim sorunu düzeltildi: Bir seçimin üzerine yazmak, siz seçimi taşıyana kadar önceki metnin kaybolmasına neden olabiliyordu.
• EPUB bölümlerinin ayrıştırılması düzeltildi: kapak veya yalnızca resim içeren sayfalar artık CSS okumaları (ör. "doldurma") veya "Bilinmeyen" başlıklar oluşturmuyor.
• EPUB'daki sesli kitapların zamana göre bölünmesiyle ilgili sorun düzeltildi: Edge TTS, boş veya çok uzun parçalarda başarısız olabiliyor ("Edge sesi gönderilmiyor").
• RSS makaleleri artık HTML varlıklarının kodunu çözüyor (ör. ", &, <, >).
• Farklı Kaydet/Farklı Kaydet, üzerine yazılamayan formatlarda (örn. EPUB) kaydederken artık ilk satır yerine mevcut dosyanın adını sunuyor.
• Yeni bölümleri olan podcast'lerin oynatılmamış olarak duyurulmamasına ve daha profesyonel olması için "oynatılmamış" ifadesinin "oynatılmamış" olarak yeniden adlandırılmasına neden olan sorun düzeltildi.

Sürüm 0.6.4 – 2026-02-05
İyileştirmeler
• Bu programın anahtarı olan ses ve sese daha fazla önem verilmesi için programın adı Sonarpad olarak değiştirildi.
• Birden fazla ses parçası içeren medya dosyaları için Oynatma menüsüne ses parçası seçimi eklendi (örn. birden fazla dil içeren MKV).
• Podcast'ler artık dinlenmeyenleri adının önüne "Dinlenmeyenler" ön ekiyle açıkça belirtiyor.
• Metindeki sesi değiştirmek için yeni etiket sistemi. Örnekler:
  - Microsoft Sesleri (Edge): <voice edge it-IT-IsabellaNeural>Merhaba</voice>
  - SAPI5 girişleri: <voice sapi5 Microsoft Helena Desktop>Hello</voice>
  - SAPI4 girişleri: <voice sapi4 #1>Hello</voice>
  - Hız/ton/ses seviyesi ile: <voice edge it-IT-ElsaNeural speed=-20 pitch=-5 volume=-10>Hello</voice>
• Zenginleştirilmiş podcast kategorileri.
• PDFium'a otomatik geri dönüş sayesinde iyileştirilmiş PDF okuma.
• Bazı durumlarda tamamı okunmayan makalelerin ayrıştırıcısı iyileştirildi.
• Oynat menüsüne perde sıfırlama eklendi.
• Seçimden sesli kitap oluşturmak için içerik menüsüne bir seçenek eklendi.
• İlk dosyanın adını seçme olanağıyla birlikte sesli kitapların süreye göre bölünmesi eklendi.
• Makaleleri okurken yazarı belirten girdi yerelleştirildi (örn. "di", "by", "par").
• Seçilen satırların girintisini/girintisini azaltmak için girinti seçenekleri (genişlikli sekme/boşluk) ve Sekme/Shift+Sekme eklendi.
• Markdown temizliği düzeltildi: artık listelerin bakımı yapılmadığında '*' işaretini de işliyor.
• Pencere başlığına ve Başlat menüsü kısayollarına eski "Novapad" adını kullanma seçeneği eklendi.
Hata düzeltmeleri
• SAPI4 içeren sesli kitapların amaçlanandan farklı şekilde oluşturulabilmesine neden olan bir hata düzeltildi.
• Aramayla sonun ötesine geçildiğinde oynatmanın baştan başlamasına neden olan bir hata düzeltildi.
• Dosyalarda Bul penceresi: Bir sonuçta Enter tuşuna basıldığında artık parçacığın doğru konumu açılıyor ve Esc sonuçlara geri dönüyor.
• Seçenekler Penceresi: Kontrollerin eksik olmasını veya kesilmesini önlemek için Genel, Ses, Düzenleyici ve Ses sekmelerinin görsel düzeni düzeltildi.
• Oynatma hızını değiştirirken oluşan yer imi sorunu düzeltildi.
• Podcast Dizini ve kategorilerin doğru görüntülenmemesiyle ilgili sorun düzeltildi.
• Kesme işaretinin okumayı bozması sorunu düzeltildi: artık diyaloglar için ayrı bir okuma yok, ses etiketleri kullanılıyor.

Sürüm 0.6.3 – 2026-01-30
İyileştirmeler
• Geliştirilmiş mikrofon algılama.
• Tüm formatlar için anında oynatma eklendi.
Düzeltmeler
• Podcast kategorileri penceresindeki kilitlenme düzeltildi.

Sürüm 0.6.2 – 2026-01-30
Yeni özellikler
• Dosya yürütme eklendi (Shift+F5). Seçenekler'den yorumlayıcıyı (örneğin python) seçebilir, bilgisayarınızda arayabilir ve Shift+F5 tuşlarına basmak mevcut komut dosyasını çalıştırır. HTML dosyaları tarayıcıda açılır.
• Varsayılan tarayıcınızda otomatik olarak açılan Google Dokümanlar işaretçi dosyaları (.gdoc, .gsheet, .gslides) için destek eklendi.
• M4B (Apple/AAC) sesli kitap formatı desteği eklendi.
• Abone olmadan bölümlere göz atmak ve oynatmak için podcast arama sonuçlarının içerik menüsüne “Bölümleri Göster” seçeneği eklendi.
• Belirli bir satır numarasına hızla atlamak için “Satıra Git” işlevi (Düzenleme menüsü veya Ctrl+J) eklendi.
• RSS beslemelerini ve podcast'leri (alfabetik veya tarihe göre) sıralamak için içerik menüsüne seçenekler eklendi.
• Vietnamca varsayılan RSS beslemeleri eklendi.
• Başlamadan önce seviyeleri kontrol etmek için kayıt penceresine bir mikrofon test kutusu eklendi.
• İçerik menüsüne podcast bölümleri için "Açıklamayı göster" eklendi.
• FFmpeg aracılığıyla genişletilmiş ses/video formatları için destek eklendi: mkv, avi, mov, m4v, webm, mpg, ts, wmv, flv, vob, 3gp, flac, ogg, wma, aiff.
• NVDA veya seçilen ses ile altyazıların (srt, vtt, ass, sub, sbv, lrc, smi) senkronize okunması eklendi. Program, medya dosyasıyla aynı adı taşıyan bir altyazı dosyası arar. Farklı adlara sahip dosyalar için Oynatma menüsüne "Altyazıları İçe Aktar" ve "Altyazıları Kaldır" seçenekleri eklendi.
• "Sonarpad ile Aç" içerik menüsüne desteklenen tüm yeni ses/video formatları için dosya ilişkilendirmeleri eklendi.
• Herhangi bir dosyanın perdesini ayarlamak için ayar eklendi.
• Genel ayarlara anonim hata raporlarını etkinleştirmek veya devre dışı bırakmak için bir onay kutusu eklendi. Tanılama ZIP dosyası oluşturmak için Yardım menüsüne bir giriş eklendi.
• Hem canlı okuma hem de sesli kitap oluşturma amacıyla diyalog için farklı bir ses kullanma seçeneği eklendi.
• Podcast'leri kategoriye göre (iş, sanat, spor vb.) keşfetmek için podcast kategorisi tarayıcısı eklendi.
İyileştirmeler
• Windows Gezgini'nden bir ses/video dosyası açıldığında artık metin düzenleyici yerine doğrudan oynatıcı görünümü açılıyor.
• Erişilemeyen PDF'ler için OCR isteği kaldırıldı; Hızı ve kullanıcı deneyimini geliştirmek için OCR artık otomatik olarak gerçekleştiriliyor.
• Geliştirilmiş Erişilebilir Terminal: NVDA okuması artık daha iyi süreklilik için okunan son satırı hatırlıyor.
• SAPI 4: Sesli kitap oluşturma artık tamamen paralelleştirilmiş ve neredeyse anında gerçekleştiriliyor. Eşzamanlı işlemlerin sayısını seçmek için bir bilgi istemi eklendi.
• SAPI 4: Sentez sırasında parçaları paralel olarak dönüştürerek WAV-MP3 darboğazını ortadan kaldırdı.
• SAPI 4: Geliştirilmiş hata yönetimi ve geçici dosyaların otomatik temizlenmesi.
• Bul Penceresi: Daha net olması açısından "Regex", "Normal İfade" olarak yeniden adlandırıldı ve arama seçeneklerine eksik çeviriler eklendi.
• M4B Sesli Kitaplar: Daha iyi çıktı yönetimi; parçalara/işaretçilere göre bölmek artık başlık ve yazar da dahil olmak üzere bölüm meta verilerini içeren tek bir M4B dosyası oluşturuyor.
• Oynatıcı: Oynatma hızı 1,0x olmadığında yer imlerinin ve zaman duyurularının doğruluğu düzeltildi.
• Seçenekler'de Ctrl+Sekme ve Ctrl+Shift+Sekme gezintisi geri yüklendi.
• Normal (1,0x) hızı anında geri yüklemek için Oynatma menüsüne bir seçenek eklendi.
• Daha iyi performans ve kararlılık için tüm bağımlılıklar en son sürümlere güncellendi.
• Başlatmayı engellemeden uyumluluğu sağlamak için dinamik DLL yüklemeli entegre FFmpeg.
• Podcast indirme filtreleri yeni ses/video formatlarını içerecek şekilde güncellendi.
• Bozulmayı önlemek için Ctrl+S'nin ses/video dosyalarını kaydetmesi engellendi.
• YouTube transkriptlerinin içe aktarımı iyileştirildi ve daha sağlam ve dayanıklı hale getirildi.
• Sesli kitapların bölünmesinin sağlamlığı iyileştirildi ve hiçbir metnin kaybolmaması sağlandı.
• Yükleyici artık tamamen çok dillidir ve kullanıcının sistem diline bağlı olarak İtalyanca, İngilizce, İspanyolca, Portekizce, İsveççe ve Vietnamca'yı desteklemektedir. Desteklenmeyen sistemler için varsayılan dil İngilizcedir.
• Podcast Kategorileri: Bir kategoride Enter'a basmak artık seçiminizi onaylıyor (OK düğmesine eşdeğer).
• Kalıcı iletişim kutuları açıkken yanlış pozitifleri (hata mesajları, "metin bulunamadı") önlemek için geliştirilmiş kilitlenme algılama sistemi.
Düzeltmeler
• Değişiklik günlüğünün başlangıçta açılmamasına neden olan bir hata düzeltildi.
• Windows Gezgini'nden açılan, erişilemeyen PDF'ler için OCR isteminin görünmemesine neden olan bir hata düzeltildi.
• Pencerelerin odağı kaybetmesine veya açıldıktan hemen sonra kapanmasına neden olabilecek bir başlatma hatası düzeltildi.
• Windows satır sonlarındaki "Dairesel Arama" ve "Nokta eşittir yeni satır" seçeneğiyle ilgili sorunlar da dahil olmak üzere, normal ifade aramasında metnin bulunmasını engelleyen kritik bir hata düzeltildi.
Yerelleştirme
• Lehçe çeviri eklendi.
• Fransızca çeviri eklendi.
• Çekçe çeviri eklendi (Radek Žalud ve Jiri Holzinger sayesinde).

Sürüm 0.6.1 – 2026-01-20
Düzeltmeler
• "Girişleri düzenleyicide göster" özelliğinin etkinleştirilmesi ve bir podcast oynatılmasının oynatmayı durdurmasına neden olan bir hata düzeltildi.
• Adres kısaltıldığı için bazı podcast'lerin URL aracılığıyla eklenememesi sorunu düzeltildi.
• RSS beslemesi işlevine artık normal URL'ler eklemenin mümkün olmadığı bir hata düzeltildi.
• Wikipedia dilinin birden fazla seçenek sekmesinde görüntülenmesine neden olan sorun düzeltildi.
• Yayın modunda da oluşturulan bazı hata ayıklama dosyalarının oluşturulması kaldırıldı.
İyileştirmeler
• Artık farklı bir kullanıcı aracısıyla özel bir mod kullanılarak oynatılan Microsoft sesleri için geliştirilmiş destek.
• MP4 dosyaları için destek eklendi.

Sürüm 0.6.0 – 2026-01-20
Yeni özellikler
• Yazım denetleyici eklendi. Bağlam menüsünden mevcut kelimenin doğru olup olmadığını kontrol edebilir ve değilse öneriler alabilirsiniz.
• Podcast'lerin OPML dosyaları yoluyla içe ve dışa aktarımı eklendi.
• iTunes'a ek olarak Podcast Dizini arama desteği eklendi. Kullanıcı, ücretsiz API anahtarını ve API sırrını girebilir (yalnızca e-posta adresini girerek oluşturulabilir).
• Hem gerçek zamanlı okuma hem de sesli kitap oluşturma amacıyla SAPI4 sesleri desteği eklendi
• Erişilemeyen PDF'ler için otomatik OCR geri dönüşü eklendi: Çıkarılabilir metin bulunmadığında belge OCR aracılığıyla tanınır.
• Vikisözlük aracılığıyla sözlük desteği eklendi. Uygulamalar düğmesine basıldığında tanımlar ve varsa diğer dillerdeki eşanlamlılar ve çeviriler görüntülenir.
• Wikipedia'dan makalelerin arama, sonuç seçimi ve doğrudan editöre aktarılmasıyla içe aktarılması eklendi.
• Bir makaleyi doğrudan orijinal web sitesinde açmak için RSS modülüne Shift+Enter kısayolu eklendi.
İyileştirmeler
• Uygulama artık mikrofon seçimine her zaman saygı gösteriyor.
• Podcast penceresinde bir bölümde Enter'a basıldığında NVDA hemen "yükleniyor" anonsunu yapar ve işlem anında onaylanır.
• Podcast arama sonuçlarında artık Enter'a basmak seçilen podcast'e abone olmanızı sağlar.
• Ctrl+Shift+O ve Podcast Ctrl+Shift+P kısayol etiketleri düzeltildi ve iyileştirildi.
• Oynatma hızı ve ses düzeyi artık ayarlara kaydediliyor ve tüm ses dosyaları için kalıcı oluyor.
• Podcast bölümleri için özel bir önbellek klasörü eklendi. Kullanıcı, Oynat menüsündeki "Podcast'leri Sakla" seçeneğini kullanarak bölümleri tutabilir. Kullanıcı tarafından belirlenen boyutu aştığında önbellek otomatik olarak boşaltılır (Seçenekler → Ses).
• Chrome ve iPhone kimliğine bürünme ile libcurl kullanılarak RSS makale alımı önemli ölçüde iyileştirildi ve sitelerin yaklaşık %99'uyla uyumluluk sağlandı.
• RSS listesinde net bir göstergeyle RSS makaleleri için okundu/okunmadı durumu eklendi.
• Tümünü Değiştir özelliği artık yapılan değişiklik sayısını da gösteriyor.
• Sekme yoluyla podcast kitaplığında gezinirken Podcast'i Sil düğmesi eklendi.
Düzeltmeler
• Yardım menüsündeki gereksiz "güncelleme bekleniyor" öğesi kaldırıldı (güncellemeler zaten otomatik olarak işleniyor).
• MP3 dosyasını açıp Ctrl+S tuşlarına basmanın dosyayı kaydetmesine ve dolayısıyla bozulmasına neden olan bir hata düzeltildi.
• Arayüzde "Toplu Sesli Kitaplar"ın "(B)... Ctrl+Shift+B" (gereksiz etiket kaldırıldı) olarak gösterilmesine neden olan sorun düzeltildi.
• Akıllı alıntıların çalışma şekli düzeltildi: Etkinleştirildiğinde, normal alıntılar artık doğru şekilde tipografik alıntılarla değiştiriliyor.
• "Yer İmine Git" seçeneğinin kullanılmasının oynatma hızını 1,0'a sıfırlamasına neden olan bir hata düzeltildi.
• Önceden indirilen podcast bölümlerinin, önbelleğe alınmış sürümü kullanmak yerine yeniden indirilmesine neden olan sorun düzeltildi.
Klavye kısayolları
• F1 artık yardımı açıyor.
• F2 artık güncellemeleri kontrol ediyor.
• F7 / F8 artık önceki veya sonraki yazım hatasına geçmenize olanak tanıyor.
• F9 / F10 artık favorilere kaydedilen girişler arasında hızla geçiş yapmanıza olanak tanıyor.
Geliştiriciler için iyileştirmeler
• Hatalar artık sessizce göz ardı edilmiyor: tüm let _ = kalıpları kaldırıldı ve hatalar artık açıkça işleniyor (yayılıyor, günlüğe kaydediliyor veya uygun geri dönüşlerle işleniyor).
• Proje artık uyarılar olması durumunda derlenmiyor: hem kargo kontrolü hem de kargo kırpılması, daha kısıtlayıcı tüy bırakmama ve mümkün olan yerlerde izinlerin kaldırılmasıyla, uyarı olmadan tamamlanmalı.
• Özel strlen / wcslen stili uygulamaları kaldırıldı. UTF-16 dizesi ve arabellek uzunlukları artık manuel bellek taramasına gerek kalmadan Rust tarafından yönetilen verilerden türetiliyor.
• DLL yönetimi temizlendi ve libloading etrafında merkezileştirildi, böylece özel yükleme mantığı ve PE ayrıştırması önlendi.
• Bayt ayrıştırma için el yapımı yardımcılar kaldırıldı: artık tüm ayrıştırma işlemleri doğrulanmış dilimlerde from_le_bytes / from_be_bytes kullanıyor.
Bu değişiklikler, güvenli olmayan ifadelerin gereksiz kullanımını azaltır, olası tanımsız davranışları ortadan kaldırır ve kodu daha deyimsel, sağlam ve bakımı kolay hale getirir.

Sürüm 0.5.9 - 2026-01-13
Yeni özellikler
• Geçersiz konumları kontrol ederek içerik menüsünden (yukarı/aşağı/konum) RSS'yi yeniden sıralama yeteneği eklendi.
• Orijinal sitenin açılması ve WhatsApp, Facebook ve aracılığıyla paylaşılmasıyla birlikte makaleler için de bağlamsal menü eklendi.
• İçe aktarılan makalelerden RSS listesine hızla dönmek için Esc kısayolu eklendi.
• Podcast modu eklendi: arayın, abone olun ve dinleyin; aboneliklerin yeniden sıralanması; Oynatmayı durdurmak ve listeye dönmek için Esc; Bir bölümün girilmesi oynatmayı başlatır.
• Podcast'ler ve MP3 dosyaları için oynatma hızı ayarı eklendi.
• Belirli bir zamana gitmek için Ctrl+T eklendi.
• Ses seviyesi kutusundan sonra bir ses önizleme düğmesi eklendi.
• Bul ve Değiştir için Notepad++ tarzında normal ifade işlevi eklendi.
• OPML ve TXT dosyalarından RSS içe aktarımı eklendi.
• Taşınabilir sürümde de Windows Gezgini'nde "Sonarpad ile Aç" seçeneğini etkinleştirmek için Seçenekler'e kutu eklendi.
• Taranan PDF'ler için OCR desteği eklendi (Windows 10/11 gerektirir): PDF metin içermiyorsa otomatik tanıma sunulur.
İyileştirmeler
• TTS'nin maksimum limitlerine uygun olarak hız, ton ve ses seviyesi seçimi iyileştirildi.
• Güncellemeler sırasında NVDA'nın odağını değiştirmeden tüm makaleleri indirmek için RSS modunda çeşitli iyileştirmeler.
• Özel bir menü ile geliştirilmiş ses çalma, Ctrl+I ile tempo duyurusu ve %300'e kadar ses seviyesi.
• Bazı işlevler için eksik kısayollar eklendi.
• Metin temizleme işlevlerine yönelik bir alt menüyle Düzenle menüsü yeniden düzenlendi.
• Sekmeler arasında geçiş yapmak için Ctrl+Sekme ve Ctrl+Shift+Sekme tuşlarını kullanarak Seçenekleri sekmeler halinde yeniden sıralayın.
• Makaleleri okurken karşılaşılan sorunlar düzeltildi: RSS okuyucu artık makaleleri tarayıcıya göre tam olarak okuyor.
Düzeltme
• Markdown temizliğinin satır başlangıcı numaralarını çıkarmasına neden olan sorun düzeltildi.
• AltGr+Z'nin Geri Al'ı tetiklemesi düzeltildi.
• Sesli kitap kaydetmenin hızlı bir şekilde durdurulamaması sorunu düzeltildi.
Yerelleştirme
• Vietnamca çeviri eklendi (Anh Đức Nguyễn'a teşekkürler).

Sürüm 0.5.8 - 2026-01-10
Yeni özellikler
• Podcast kaydı sırasında mikrofon ve sistem sesi için ses seviyesi kontrolü eklendi.
• Her dil için en önemli beslemeler de dahil olmak üzere, web sitelerinden veya RSS beslemelerinden makaleleri içe aktarmak için yeni bir işlev eklendi.
• Geçerli dosyanın tüm yer imlerini kaldırma işlevi eklendi.
• Yinelenen satırları ve ardışık yinelenen satırları kaldırma işlevi eklendi.
• Geçerli olan dışındaki tüm sekmeleri veya pencereleri kapatma işlevi eklendi.
• Tüm diller için Yardım menüsüne Bağışlar öğesi eklendi.
İyileştirmeler
• Erişilebilir terminal iyileştirildi ve bazı çökmeler önlendi.
• Programın erişim tuşları ve klavye kısayolları iyileştirildi ve düzeltildi.
• Ses oynatma penceresini kapatmanın oynatmayı durdurmaması sorunu düzeltildi.
• Önemli eylemler için onay pencereleri eklendi (örneğin, yinelenen satırların kaldırılması, satır sonu çizgilerinin kaldırılması, mevcut dosyadaki tüm yer imlerinin kaldırılması). Eylem geçerli değilse onay yok.
• RSS beslemelerini/sitelerini seçip Sil'e basarak kitaplıktan silme özelliği eklendi.
• RSS beslemelerini/sitelerini düzenlemek veya silmek için RSS penceresine bir içerik menüsü eklendi.
• Ayarları geçerli klasöre taşımak için onay kutusu kaldırıldı: artık program bunu otomatik olarak yönetiyor (exe klasörü "sonarpad taşınabilir" olarak adlandırılıyorsa veya exe çıkarılabilir bir sürücüdeyse, exe klasörüne "config" dosyasına, aksi takdirde "%APPDATA%\\Sonarpad" dosyasına, tercih edilen klasör yazılabilir değilse `config'e geri dönüşle kaydedin).

Sürüm 0.5.7 - 2026-01-05
Yeni özellikler
• Sesli kitapları toplu olarak kaydetme seçeneği eklendi (birden fazla dosya ve klasör dönüştürme).
• Markdown (.md) dosyaları için destek eklendi.
• Metin dosyalarını açarken kodlama (kodlama) seçeneği eklendi.
• NVDA ile yeni gelen hatları duyurmak için terminale seçenek eklendi.
İyileştirmeler
• Kayıtlar (sesli kitaplar) artık seçildiğinde yerel MP3 formatında kaydediliyor.
• Kullanıcı kaydedilmemiş değişiklikleri gösteren yıldız işaretinin * nereye ekleneceğini seçebilir (pencere başlığı).
• Farklı senaryolarda daha sağlam hale getirmek için yükseltme sistemi iyileştirildi.
• Düzenle menüsüne satır sonundaki tireleri kaldırma işlevi eklendi (OCR metinleri için kullanışlıdır).

Sürüm 0.5.6 - 2026-01-04
Düzeltme
  Dosyalarda geliştirilmiş Bul: Enter tuşuna basmak, dosyayı tam olarak seçilen parçacığın bulunduğu konumda açar.
İyileştirmeler
  PPT/PPTX desteği eklendi.
  Metin dışı formatlar için Şimdi Kaydet, formatın bozulmasını önlemek amacıyla her zaman .txt dosyasını önerir (PDF/DOC/DOCX/EPUB/HTML/PPT/PPTX).
  Mikrofondan ve sistem sesinden podcast kaydı eklendi (Dosya menüsü, Ctrl+Shift+R).

Sürüm 0.5.5 - 2026-01-03
Yeni özellikler
• Ekran okuyuculara çok fazla çıktı gönderen programlar için optimize edilmiş erişilebilir bir terminal eklendi (Ctrl+Shift+P).
• Kullanıcı ayarlarını geçerli klasöre kaydetme seçeneği eklendi (taşınabilir mod).
Düzeltme
• Önizlemeyi eşleşecek şekilde hizalamak için Dosyalarda Bul parçacıkları iyileştirildi.

Sürüm 0.5.4 – 2026-01-03
İyileştirmeler
• Boşlukları Normalleştirme (Ctrl+Shift+Enter) işlevini düzeltin.
• HTML/HTM desteği eklendi (metin olarak açık).

Sürüm 0.5.3 – 2026-01-02
Yeni özellikler
• Dosyalarda Bul eklendi.
• Yeni metin araçları eklendi: Boşlukları Normalleştir, Satırları Yeniden Biçimlendir ve İşaretleme Metnini Temizle.
• Metin İstatistikleri eklendi (Alt+Y).
• Düzen menüsüne yeni liste komutları eklendi:
• Satırları sıralama (Alt+Shift+O)
• Kopyaları kaldırın (Alt+Shift+K)
• Satırları ters çevir (Alt+Shift+Z)
• Yorum yapma / Yorumu kaldırma satırları eklendi (Ctrl+Q / Ctrl+Shift+Q).
Yerelleştirme
• İspanyolca dili eklendi.
• Portekizce dili eklendi.
İyileştirmeler
• Bir EPUB dosyası açıldığında Kaydet, otomatik olarak Farklı Kaydet'e geçer ve EPUB bozulmasını önlemek için içeriği .txt olarak dışa aktarır.

## 0.5.2 - 2026-01-01

* Değişiklik günlüğü eklendi.
* Kurulum sırasında desteklenen dosyalar için "Sonarpad ile aç" seçenekleri ve ilişkilendirmeleri eklendi.
* Geliştirilmiş mesaj yerelleştirmesi (hatalar, diyaloglar, sesli kitap aktarımı).
* "Sesli kitabı metne göre böl" seçeneği kullanıldığında "Metin yeni satırdan başlamalıdır" seçeneğiyle parça seçimi eklendi.
* Dil seçimi, zaman damgası seçeneği ve odak yönetimi ile YouTube'dan transkriptlerin içe aktarılması eklendi.

## 0.5.1 - 2025-12-31

* Onaylama, hata yönetimi ve geliştirilmiş bildirimlerle otomatik güncelleme.
* Geliştirilmiş sesli kitap dışa aktarma (metin için bölme, SAPI5/Media Foundation, gelişmiş kontroller).
* TTS iyileştirmeleri (duraklatma/devam ettirme, sözlük değiştirme, favoriler).
* Menüyü ve öğe/favori panelleri, metin rengini ve boyutunu görüntüleyin.
* Sistem varsayılan dili ve yerelleştirme iyileştirmeleri.
* CI ve Windows paketleme (yapılar, MSI/NSIS, önbellek).

## 0.5.0 - 2025-12-27

* Modüler refactor (düzenleyici, dosya işleyicisi, menü, arama).
* Windows derleme/paketleme iş akışı ve README/lisans güncellemeleri.
* Yardım penceresinde SEKME gezinmesini düzeltin.

## 0.5 - 2025-12-27

* Ön sürüm numarası güncellendi.

## 0.1.0 - 2025-12-25

* İlk versiyon: proje yapısı ve ilk README.
