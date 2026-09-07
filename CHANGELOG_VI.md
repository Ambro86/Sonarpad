# Nhật ký thay đổi

Phiên bản 0.9.5 – 2026-09-07

Mô tả âm thanh bằng AI
1. Cải thiện độ ổn định khi Gemini tạm thời trả về `MALFORMED_RESPONSE` mà không có nội dung sử dụng được. Sonarpad giờ sẽ thử lại đúng đoạn đó tối đa ba lần, với một khoảng chờ ngắn giữa các lần thử, thay vì dừng toàn bộ quá trình mô tả âm thanh ngay lập tức. Các phản hồi Gemini bình thường, chặn an toàn, lỗi giới hạn token và cách hoạt động hiện tại của checkpoint vẫn không thay đổi.

2. Đã sửa thêm một trường hợp hiếm gặp của lỗi `invalid Gemini chunk timeline` với một số video có các đoạn do FFmpeg chuẩn bị báo cáo độ lệch thời lượng tích lũy nhỏ. Sonarpad giữ nguyên luồng xử lý hiện tại khi timeline hợp lệ và chỉ dùng cơ chế căn chỉnh thận trọng khi timeline thông thường sẽ thất bại và tổng độ lệch nhỏ; các sai lệch lớn hoặc đáng ngờ vẫn tiếp tục báo lỗi.

3. Đã thêm dịch vụ Sonarpad AI tùy chọn cho mô tả âm thanh bằng AI. Người dùng vẫn có thể dùng khóa API Gemini của riêng mình hoặc chọn dịch vụ Sonarpad và nhập mã Sonarpad cá nhân. Mã được bảo vệ bằng Windows DPAPI. Trong chế độ dịch vụ, các đoạn video đã chuẩn bị được tải trực tiếp từ PC của người dùng lên Google thông qua URL tải lên Google tạm thời; sonarpad.com không nhận hoặc lưu trữ dữ liệu video. Backend chỉ xác thực quyền truy cập, áp dụng mô hình Gemini/cấp Standard và giới hạn sử dụng, ghi nhận mức tiêu thụ và trả về phản hồi Gemini.

4. Cải thiện việc chỉnh sửa dự án thuyết minh âm thanh đã lưu. Giờ đây có thể sửa nhiều mô tả liên tiếp mà không cần áp dụng từng thay đổi ngay lập tức. Các thay đổi được giữ trong bộ nhớ; khi nhấn “Áp dụng văn bản”, Sonarpad kiểm tra từng mô tả đã sửa với khoảng thời gian đã lưu rồi lưu tất cả thay đổi hợp lệ cùng lúc. Nếu một mô tả không vừa trong khoảng của nó, tiêu điểm sẽ quay lại mô tả đó mà không làm mất các thay đổi đang chờ khác.

5. Đã thêm “Điều chỉnh giọng nói” ngay trước “Tạo thuyết minh âm thanh”. Cửa sổ mới cho phép chọn công cụ tổng hợp, giọng nói, tốc độ và âm lượng, đồng thời có “Thử giọng”. Khi nhấn OK, tiêu điểm quay lại chính xác “Điều chỉnh giọng nói” và các giá trị đã chọn được áp dụng cho thuyết minh âm thanh sắp tạo. Nếu không dùng cửa sổ này, cách tổng hợp vẫn giữ nguyên như trước.


6. Đã mở rộng các điều khiển giọng nói trong dự án mô tả âm thanh đã lưu. Trong Sửa dự án, ngoài bộ máy và giọng nói, giờ đây còn có tốc độ, âm lượng và “Thử giọng”. Khi áp dụng các tham số giọng nói, Sonarpad kiểm tra lại toàn bộ mô tả hiện có bằng các tham số tổng hợp mới và chỉ tạo lại MP3 nếu tất cả mô tả vẫn nằm trong các khoảng thời gian đã lưu. Các khoảng không có hội thoại, thời điểm Visual Evidence, mô tả Gemini và phân tích thời gian của dự án được tái sử dụng mà không chạy lại phân tích AI.

Phiên bản 0.9.4 – 2026-09-04

Mô tả âm thanh bằng AI
1. Đã sửa lỗi với video Matroska/WebM có dấu thời gian bắt đầu nội bộ cao bất thường, có thể làm quá trình tạo mô tả âm thanh dừng lại với lỗi “invalid Gemini chunk timeline”. Sonarpad giờ chỉ chuẩn hóa thời lượng của tệp nguồn và các đoạn Gemini khi phát hiện kiểu dấu thời gian bất thường này, còn video thông thường và cách hoạt động mô tả âm thanh vốn đã ổn định vẫn không thay đổi.
2. Cải thiện ducking của mô tả âm thanh để bản trộn nghe tự nhiên hơn. Âm thanh gốc giờ được giảm nhẹ trước khi giọng mô tả bắt đầu và trở lại mức bình thường một cách từ từ sau phần mô tả; các đoạn mô tả nằm rất gần nhau sẽ giữ mức nền ổn định, tránh việc âm lượng liên tục hạ xuống rồi tăng lên.
3. Đã sửa lỗi có thể làm quá trình xuất mô tả âm thanh cuối cùng thất bại với một số tệp AVI và các nguồn khác dùng âm thanh 5.1/đa kênh khi quá trình giải mã kết thúc bằng một khung âm thanh xen kẽ chưa đầy đủ. Sonarpad giờ chỉ bổ sung an toàn cho khung cuối cùng bị thiếu đó bằng đúng số mẫu im lặng cần thiết; các khung đã đầy đủ và các tệp mono, stereo hoặc đa kênh thông thường vẫn không thay đổi.


Phiên bản 0.9.3 – 2026-09-03

Giọng SAPI5
1. Đã sửa lỗi khiến một số giọng SAPI5 cục bộ có thể không phát tiếng khi bật di chuyển con trỏ, khi đọc nhiều giọng hoặc khi tạo sách nói/MP3. Sonarpad giờ dùng một đường tổng hợp SAPI5 ra tệp hoạt động ổn định trên Windows và vẫn cho phép hủy trong khi tổng hợp, còn phát trực tiếp thông thường không thay đổi.
2. Đã sửa vị trí con trỏ khi đọc hội thoại bằng nhiều giọng. Các thẻ giọng nói do Sonarpad tự động chèn cho hội thoại giờ chỉ được xem là siêu dữ liệu phát lại, không còn được tính là ký tự trong trình soạn thảo, vì vậy sau F4 hoặc F6 con trỏ không còn nhảy lên trước văn bản thật. Việc đọc bằng một giọng và các thẻ <voice> được viết rõ trong tài liệu vẫn không thay đổi.

Mô tả âm thanh bằng AI
1. Đã thêm hộp kiểm “Hiển thị khóa API” ngay sau trường khóa API Gemini. Theo mặc định hộp này tắt; khi bật, toàn bộ khóa sẽ tạm thời được hiển thị để người dùng kiểm tra xem khóa đã được dán đầy đủ hay chưa. Khi mở lại cửa sổ, khóa luôn được ẩn trở lại.

Podcast và Wikipedia
1. Sau khi lưu bản ghi podcast, Sonarpad giờ sẽ hỏi có muốn mở thư mục chứa tệp đã lưu hay không, giống như sau khi lưu nội dung YouTube/streaming.
2. Khi nhập thêm một bài viết Wikipedia vào trình soạn thảo đã có văn bản, bài viết mới giờ được thêm vào cuối thay vì chèn lên đầu. Con trỏ được đặt ở đầu bài viết vừa nhập.


Phiên bản 0.9.2 – 2026-09-02

Mô tả âm thanh bằng AI
1. Đã sửa lỗi có thể khiến mô tả âm thanh bằng AI thất bại trong bước xuất MP3 cuối cùng với các video có âm thanh đa kênh, chẳng hạn như 5.1. Sonarpad giờ tự động chuyển âm thanh đa kênh xuống stereo chỉ khi cần cho việc mã hóa MP3, không thay đổi các bản xuất mono hoặc stereo.
2. Khi bắt đầu tạo mô tả âm thanh bằng AI cho video có nhiều bản âm thanh, Sonarpad giờ sẽ hỏi bản nào cần dùng trước khi xử lý. Hộp tổ hợp có thể truy cập được có thể thay đổi bằng các phím mũi tên; OK bắt đầu mô tả âm thanh với bản đã chọn, còn Hủy sẽ đóng cửa sổ mô tả âm thanh và đưa tiêu điểm trở lại trình soạn thảo Sonarpad.

YouTube và phát trực tuyến
1. Đã sửa lỗi khiến khi bắt đầu mô tả âm thanh bằng AI cho video ở trang 2 hoặc các trang sau của danh sách phát hay kênh YouTube, cửa sổ chọn YouTube có thể mở lại và lấy tiêu điểm khỏi cửa sổ mô tả âm thanh. Sonarpad giờ đóng bộ chọn đúng cách mà không quay lại các trang trước.
Phiên bản 0.9.1 – 2026-09-01

Tải xuống YouTube
• Đã sửa lỗi khiến cửa sổ tiến trình tải YouTube/streaming có thể liên tục trở lại phía trước sau khi chuyển sang ứng dụng khác bằng Alt+Tab. Việc tải xuống giờ tiếp tục ở chế độ nền mà không giành lại tiêu điểm.
• Cải thiện khả năng tiếp cận của tiến trình tải xuống. Khi quay lại cửa sổ tiến trình, trình đọc màn hình có thể đọc trạng thái hiện tại và phần trăm. Với danh sách phát, Sonarpad cũng thông báo số thứ tự mục hiện tại, tổng số mục và tiêu đề.
• Đã sửa các cảnh báo treo giả từ watchdog trong các lần tải xuống và chuyển đổi dài khi cửa sổ tiến trình vẫn phản hồi.
• Đã thêm hộp kết hợp Định dạng cho tải xuống danh sách phát. Từ danh sách video, nhấn Tab để chọn MP4, MP3, M4A, OPUS, OGG, WAV hoặc FLAC trước khi bắt đầu tải xuống nhiều mục.
• Đã tổ chức lại việc lưu nội dung streaming. Định dạng và chất lượng giờ được chọn khi lưu thay vì trong cửa sổ tìm kiếm streaming ban đầu. “Lưu media” mở một hộp thoại chung cho Định dạng và Chất lượng, còn tải playlist có cả hai hộp kết hợp.

Mô tả âm thanh bằng AI
• Đã sửa lỗi có thể khiến mô tả âm thanh bằng AI không khởi động với một số video MKV. Sonarpad giờ xử lý đáng tin cậy hơn các video có dấu thời gian không đều hoặc bị thiếu.

Phiên bản 0.9.0 – 2026-08-31

Mô tả âm thanh bằng AI — tính năng chính mới
• Đã thêm “Tạo mô tả âm thanh bằng AI” trong Công cụ > Đa phương tiện. Sonarpad phân tích âm thanh để tìm các khoảng không có lời thoại, tạo mô tả bằng Gemini và dùng các bộ máy giọng nói đã có, tránh nói chồng lên hội thoại.
• Cải thiện đồng bộ giữa nội dung đang xảy ra trong video và phần mô tả, đồng thời tự động kiểm tra thời gian do Gemini tạo.
• “Bật khoảng dừng mở rộng” mặc định không được chọn. Có thể bật tùy chọn này với nội dung có nhiều hội thoại hoặc ít khoảng trống để chèn được các mô tả dài hơn.
• Sonarpad có thể thử nhận diện nhân vật và dùng tên của họ. Danh mục nhân vật có thể được giữ giữa các tập của một loạt phim để cải thiện tính liên tục.
• Có thể lưu dự án, chỉnh sửa mô tả sau đó và xuất lại mà không phải tạo lại toàn bộ bằng Gemini.
• Nếu quá trình bị gián đoạn, Sonarpad giữ lại tiến độ và cho phép tiếp tục mô tả âm thanh. Khi hết hạn mức Gemini, có thể chờ, đổi mô hình hoặc dừng mà không mất phần công việc đã hoàn thành.
• Cửa sổ cho phép chọn ngôn ngữ, mức chi tiết, mô hình Gemini, bộ máy giọng nói và giọng đọc, đồng thời ghi nhớ các tùy chọn đã dùng.
• Mô-đun có sẵn trong cả 17 ngôn ngữ của Sonarpad. Trong lúc tạo, giao diện chỉ hiển thị tiến độ, trạng thái hiện tại và Hủy; khi hoàn tất có thể mở MP3 trực tiếp trong trình phát nội bộ.

Sách điện tử và tài liệu
• Đã thêm nhập Kindle không DRM ở các định dạng MOBI, AZW và AZW3, với văn bản và chương có trong trình soạn thảo và chỉ mục.
• Đã thêm hỗ trợ DAISY 2.02 và DAISY 3. Sách nói DAISY dùng trình phát nội bộ của Sonarpad và tôn trọng điều hướng cũng như giới hạn chương.
• Kindle và DAISY được nhập mà không ghi đè tệp gốc; Kindle có DRM bị từ chối rõ ràng.
• Đã sửa “Lưu thành” cho EPUB: khi chọn TXT hoặc định dạng khác, phần mở rộng đã chọn giờ được dùng và EPUB gốc vẫn gắn với tài liệu đang mở.

RSS và bài viết
• Đã thêm chọn nhiều bài viết RSS để xóa nhiều bài trong một thao tác.
• RSS giờ hỗ trợ các thư mục thực sự được giữ nguyên khi nhập và xuất OPML, kể cả thư mục trống.
• Các feed có thể được sắp xếp lại trong thư mục hiện tại bằng Di chuyển lên, Di chuyển xuống, Di chuyển lên đầu, Di chuyển xuống cuối và Di chuyển đến vị trí.

Khả năng tiếp cận, hướng dẫn và giao diện
• Các hướng dẫn Sonarpad đã được sắp xếp lại với mục lục và bổ sung hướng dẫn đầy đủ về Mô tả âm thanh bằng AI.
• Đã sửa lỗi bản dịch tiếng Đức có thể khiến Mở, Lưu thành và các hộp thoại chọn tệp khác không xuất hiện.

Giọng nói và ngôn ngữ
• Danh mục Google TTS có thể tải xuống đã tăng từ 104 lên 156 gói và từ 53 lên 81 biến thể ngôn ngữ.
• Đã thêm các gói Google TTS mới và tên bản địa hóa cho nhiều ngôn ngữ hơn trong toàn bộ giao diện.

Phiên bản 0.8.4 – 2026-07-24

Chỉnh sửa tài liệu EPUB
• Sonarpad giờ đây không chỉ mở được tài liệu EPUB mà còn có thể chỉnh sửa và lưu lại ở định dạng EPUB, đồng thời giữ nguyên định dạng ban đầu, mục lục, chú thích cuối trang, hình ảnh, bảng kiểu, siêu dữ liệu và các liên kết nội bộ.
• Định dạng EPUB có trong hộp thoại “Lưu thành” đối với tài liệu được mở từ tệp EPUB. Khi lưu, chỉ phần văn bản đã thay đổi được cập nhật và cấu trúc của sách vẫn được giữ nguyên.

Độ tin cậy của sách nói
• Đã sửa lỗi không liên tục khiến một đơn vị tổng hợp bị âm thầm loại bỏ sau năm lần Google TTS thất bại, làm sách nói cuối cùng có thể thiếu một phần văn bản.
• Các đơn vị Google giờ được thử lại cho đến khi thành công hoặc người dùng hủy. Việc khởi động các tiến trình được giãn cách để giảm xung đột tạm thời với Chrome và tệp; Sonarpad cũng dừng quá trình thay vì lưu một sách nói bị thiếu đoạn.
• Sách nói Edge giờ sẽ thử lại không giới hạn cố định đối với các lỗi tạm thời về mạng, WebSocket, hết thời gian chờ, giới hạn dịch vụ và âm thanh không hợp lệ, cho đến khi thành công hoặc người dùng hủy, kể cả khi dùng giọng hỗn hợp và chia theo thời lượng. SAPI4 và SAPI5 vẫn dùng cơ chế thử lại thích ứng nhưng hữu hạn; nếu một đoạn tiếp tục thất bại, Sonarpad sẽ dừng mà không lưu sách nói không đầy đủ.

Điều hướng thư viện số
• Kết quả của LibriVox, Internet Archive và Project Gutenberg giờ dùng điều hướng theo trang giống YouTube: “Quay lại kết quả trước” nằm ở đầu danh sách và “Chuyển đến kết quả tiếp theo” nằm ở cuối.
• Đã sửa việc chuyển tiêu điểm trong LibriVox: khi mở sách hoặc chương, tiêu điểm NVDA không còn chuyển về trình soạn thảo chính trước khi danh sách tiếp theo hoặc trình phát được mở.
• Đã thêm cơ chế bảo vệ tiêu điểm trong khi tìm kiếm và tải sách LibriVox: cửa sổ tải đã bản địa hóa luôn ở phía trước trong suốt yêu cầu, ngăn tiêu điểm NVDA chuyển sang Command Prompt, Windows Terminal hoặc ứng dụng khác.

Tải danh sách phát YouTube
• Đã thêm lệnh chọn nhiều mục có khả năng truy cập cho danh sách phát YouTube, cho phép chọn video cần tải mà không thay đổi lệnh “Lưu phương tiện” hiện có của mục đang phát.
• Các mục đã chọn được tải lần lượt bằng định dạng và chất lượng đã chọn khi mở danh sách phát, được đặt tên có số thứ tự giữ nguyên trật tự ban đầu và được lưu trong một thư mục riêng bên trong thư mục Phương tiện đã cấu hình.
• Cửa sổ có các lệnh “Chọn tất cả” và “Bỏ chọn tất cả”, thông báo số mục đã chọn, cho phép hủy mà vẫn giữ các tệp đã hoàn tất và báo rõ các mục không thể tải xuống.
• Các mục trong danh sách phát giờ là hộp kiểm gốc của Windows: trình đọc màn hình tự động thông báo tiêu đề, loại điều khiển và trạng thái đã chọn hoặc chưa chọn, không thêm từ vào tiêu đề hiển thị và không dùng thông báo giọng nói cưỡng bức.

Phiên bản 0.8.3 – 2026-07-23

Chế độ tối
• Đã thêm chế độ tối, có thể bật trong menu Xem và được lưu trong tùy chọn của người dùng.
• Giao diện tối được áp dụng cho trình soạn thảo, menu, cửa sổ phụ và các điều khiển chính; màu chữ được điều chỉnh để duy trì khả năng đọc và trợ năng.

Ngôn ngữ tiếng Đức
• Đã thêm tiếng Đức làm ngôn ngữ giao diện hoàn chỉnh, có thể chọn trong Tùy chọn.
• Tin tức và RSS, kiểm tra chính tả, lịch cùng toàn bộ trích dẫn, quyên góp, hướng dẫn và nhật ký thay đổi đều có đầy đủ bằng tiếng Đức.

Tiếng Bồ Đào Nha Brazil và Google Tin tức
• Đã thêm tiếng Bồ Đào Nha Brazil như một ngôn ngữ giao diện đầy đủ, tách biệt với tiếng Bồ Đào Nha tại Bồ Đào Nha và có thể chọn trong Tùy chọn.
• Toàn bộ giao diện, lịch và mọi câu trích dẫn, kiểm tra chính tả, thông tin quyên góp, hướng dẫn và nhật ký thay đổi đều có bản tiếng Bồ Đào Nha Brazil.
• Google Tin tức hiện hỗ trợ bản địa hóa Brazil, các danh mục Brazil và các nguồn RSS Brazil mặc định riêng biệt.
• Khi nguồn cấp cung cấp, các nguồn liên quan của cùng một tin được hiển thị dưới dạng mục con có thể truy cập trong cây.

LibriVox
• Đã tối ưu hóa tìm kiếm LibriVox để tránh gửi quá nhiều yêu cầu đến dịch vụ và làm treo giao diện. Việc quét danh mục trên diện rộng đã được loại bỏ, số lần thử được giảm và thời gian chờ ngắn hơn được áp dụng.

Tổng hợp giọng nói
• Các chuỗi gồm ba dấu chấm trở lên giờ được chuẩn hóa trước khi đọc, tránh việc một số giọng phát âm “chấm chấm” hoặc tạo ra các đoạn chỉ gồm dấu câu.

Bài viết liên quan trên Google Tin tức
• Với mỗi tin tức, các bài viết liên quan sẽ được hiển thị khi có, tức là những bài viết khác nói về cùng một tin. Để đọc chúng, chỉ cần mở rộng bài viết chính khi Sonarpad thông báo rằng có bài viết liên quan. Nếu không muốn mở rộng phần này, chỉ cần nhấn Enter trên bài viết chính và đọc tin như vẫn làm trước đây.
• Các bài viết liên quan giờ sử dụng cùng hệ thống đã đọc/chưa đọc như bài viết chính, bao gồm thông báo hỗ trợ tiếp cận, ngày giờ, lưu trạng thái và giữ nguyên trạng thái sau khi cập nhật nguồn hoặc khởi động lại Sonarpad.

Thông báo trong các phần sách nói
• Đã thêm hộp kết hợp “Thông báo ở đầu mỗi phần” vào Tùy chọn âm thanh. Với sách nói được chia thành nhiều tệp, mỗi phần có thể bắt đầu mà không có thông báo, bằng tên sách, tên sách và số phần, tên tệp hoặc tên tệp và số phần.

Phiên bản 0.8.2 – 2026-07-17

Thư viện số và sách nói
• Đã thêm Project Gutenberg, cho phép tìm kiếm theo tiêu đề hoặc tác giả và chọn ngôn ngữ.
• Sách EPUB từ Project Gutenberg được tải xuống thư mục Documents\Sonarpad\Documents; khi hoàn tất, Sonarpad sẽ hỏi có muốn mở sách ngay trong trình soạn thảo hay không.
• Đã thêm Internet Archive để tìm kiếm và nghe các bộ sưu tập âm thanh, bao gồm chương trình phát thanh cũ, bài phát biểu và nhạc trực tiếp.
• Đã thêm LibriVox để tìm sách nói theo tiêu đề hoặc tác giả và phát trực tiếp từng chương bằng cùng trình phát được dùng cho podcast.
• Ba chức năng mới có trong menu Công cụ và, khi bật nhóm menu, trong phần Đọc.

Phiên âm thanh dài
• Đã sửa lỗi phiên âm các tệp âm thanh dài: âm thanh giờ được tự động chia thành các phần 15 phút, phiên âm lần lượt rồi ghép lại, tránh các lỗi có thể xảy ra với bản ghi dài.

YouTube
• Các thao tác hữu ích nhất trước đây chỉ có thể truy cập sau khi mở video YouTube và vào menu Phát giờ cũng có sẵn trực tiếp trong menu ngữ cảnh của chính video đó, chẳng hạn như “Chuyển âm thanh hiện tại thành văn bản”, “Tạo thuyết minh hình ảnh bằng AI” và “Lưu media”, giúp sử dụng thuận tiện hơn.
• Đã thêm lệnh “Sao chép liên kết”, cũng có thể dùng bằng Ctrl+C, để sao chép URL của video, danh sách phát hoặc kênh YouTube đang chọn vào bảng tạm.

Phiên bản 0.8.1 – 2026-07-16

Tổng hợp giọng nói Google
• Đã sửa lỗi khởi động Google TTS trên một số hệ thống Windows, nơi các kết nối được máy chủ trình duyệt nội bộ chấp nhận kế thừa chế độ socket không chặn, gây lỗi 10035 và khiến các giọng đã tải xuống không thể phát âm.
• Sonarpad giờ chờ bộ máy WASM của Chrome hoặc Edge tải xong hoàn toàn trước khi nghe thử giọng hoặc đọc bằng F5, tránh lỗi “Chrome WASM TTS engine was not loaded”.
• Trình duyệt ẩn tắt tính năng dịch trang và khả năng truy cập của tiến trình kết xuất để tránh thông báo như “Dịch trang” và không gây nhiễu các lệnh đọc.
• Bảng “Giọng nói trong trình soạn thảo” giờ hiển thị nút “Quản lý giọng nói Google...” khi chọn bộ máy Google và cập nhật ngay danh sách giọng đã cài đặt sau khi đóng trình quản lý.
• Cảnh báo về gói phụ thuộc khi xóa gói giọng Google hiện đã được dịch sang tất cả ngôn ngữ giao diện.

Trải nghiệm cập nhật
• Sau khi cập nhật tự động, cửa sổ hoàn tất kèm nhật ký thay đổi sẽ mở sau khi việc khôi phục tiêu điểm ban đầu kết thúc và luôn ở phía trước, thay vì chỉ xuất hiện sau khi nhấn Tab.

Tài liệu PDF
• Đã sửa các tệp PDF có văn bản nhúng chứa ký tự NUL khiến nội dung bị cắt tại ký tự đầu tiên khi tải vào trình soạn thảo.
• Khi pdf-extract trả về ký tự NUL nhúng, Sonarpad sẽ thử lại bằng PDFium; mọi ký tự NUL còn lại được loại bỏ trước khi gửi văn bản tới các điều khiển Windows, nhờ đó phần còn lại của tài liệu được giữ nguyên.

Khả năng truy cập của menu
• Đã loại bỏ việc tính ký tự gợi nhớ trong lúc chạy: phím truy cập giờ được ghi rõ trong cả 15 bản dịch giao diện và luôn giữ nguyên giữa các lần khởi động.
• Đã kiểm tra mọi mục ổn định trong menu chính và menu con, bao gồm Phát, lựa chọn phông chữ, Lưu hình ảnh và Hiển thị mục lục EPUB; các ký tự gợi nhớ bị thiếu hoặc trùng giữa các mục cùng cấp được sửa trực tiếp trong bản dịch.
• Các kiểm thử tự động giờ chỉ xác thực bản dịch và sẽ thất bại nếu ký tự gợi nhớ bị thiếu, không hợp lệ hoặc trùng; chúng không còn thay đổi nhãn menu trong lúc chạy.
• Với menu đặc biệt dài mà nội dung dịch không cung cấp đủ ký tự khác nhau, một phím truy cập dạng số rõ ràng sẽ được hiển thị theo định dạng Windows chuẩn “(&1)”.

Phiên bản 0.8.0 – 2026-07-15

Từ điển trực tuyến
• Đã thêm tiếng Đức vào từ điển trực tuyến Wiktionary.
• Định nghĩa và từ đồng nghĩa tiếng Đức giờ được nhận diện đúng theo cấu trúc riêng của Wiktionary tiếng Đức.

Độ tin cậy của sách nói SAPI5
• Việc tạo sách nói SAPI5 vẫn sử dụng tối đa 12 worker song song khi giọng đã chọn tạo kết quả đáng tin cậy.
• Mỗi phần được kiểm tra theo kích thước tệp, thời lượng ước tính và phép so sánh thận trọng với văn bản được giao.
• Các phần bị thiếu hoặc đáng ngờ sẽ tự động được tạo lại với mức song song giảm dần: 12, 8, 6, 4, 2 và cuối cùng là 1 worker. Chỉ các phần có vấn đề mới được lặp lại.
• Giới hạn đáng tin cậy được ghi nhớ riêng cho từng giọng SAPI5, không làm chậm các giọng hoạt động đúng với 12 worker.
• Kiểm tra cuối ngăn Sonarpad âm thầm chấp nhận tệp MP3 ngắn hơn nhiều so với các phần đã tạo.
• Chi tiết được ghi vào `sapi5_audiobook_diagnostic.log`.
• Mỗi đơn vị tổng hợp SAPI5 giờ chạy trong một tiến trình Sonarpad riêng và ẩn. Nếu giọng của bên thứ ba bị lỗi, chỉ worker đó đóng lại còn ứng dụng chính vẫn mở.
• Ngay trong lần tạo sách nói hiện tại, các phần chưa hoàn tất được thử lại ngay với mức song song thấp hơn kế tiếp; các phần đã được xác thực vẫn được giữ lại.
• Khôi phục ở lần khởi động tiếp theo chỉ còn là lớp bảo vệ bổ sung khi ứng dụng chính hoặc máy tính bị gián đoạn.

Tiến trình sách nói SAPI4
• Số tiến trình SAPI4 do người dùng chọn giờ được tôn trọng tới giới hạn kỹ thuật 64; giới hạn ẩn 16 trước đây đã bị loại bỏ.
• Số lượng thực tế chỉ giảm khi sách nói có ít đơn vị công việc hơn số đã yêu cầu.
• Nếu một hoặc nhiều tiến trình cầu nối SAPI4 thất bại, các phần đã hoàn tất được giữ lại và chỉ các đơn vị lỗi được tự động thử lại với mức song song giảm dần.
• Sonarpad giờ kiểm tra mã thoát của cầu nối SAPI4 và từ chối các phần âm thanh rỗng hoặc không hợp lệ.

Cấu hình proxy
• Đã thêm trường riêng cho cổng proxy trong phần cài đặt mạng.
• Cổng có thể được nhập độc lập với địa chỉ proxy, được kiểm tra trong khoảng từ 1 đến 65535 và thay thế đúng cổng đã có trong URL.

Tìm radio theo ngôn ngữ và quốc gia
• Bộ lọc Ngôn ngữ và Quốc gia giờ được cập nhật bằng tất cả các mục có trong danh mục Radio Browser thay vì bị giới hạn trong một danh sách cố định.
• Tên ngôn ngữ giờ được nhận diện ngay cả khi Radio Browser cung cấp bằng hệ chữ khác, tên bản địa, dạng viết tắt hoặc tổ hợp nhiều ngôn ngữ, rồi được hiển thị bằng ngôn ngữ giao diện hiện tại. Các giá trị không phải ngôn ngữ thực, chẳng hạn số, thể loại nhạc, quốc gia hoặc nhãn chung, sẽ bị lọc bỏ.
• Danh mục được cập nhật trong nền và vẫn có danh sách dự phòng khi không thể kết nối với Radio Browser.
• Các mục ngôn ngữ Radio Browser trở nên giống hệt nhau sau khi dịch giờ được gộp thành một mục duy nhất trong hộp danh sách, tránh các bước im lặng với trình đọc màn hình.

Cải tiến quan trọng: đồng bộ giữa giọng đọc và con trỏ
• Khả năng đồng bộ giữa giọng đọc và việc di chuyển con trỏ đã được cải thiện đáng kể cho tất cả các bộ máy giọng nói được hỗ trợ.
• Khi bật tùy chọn “Di chuyển con trỏ trong khi đọc”, Sonarpad sử dụng một hệ thống tiến trình chung cho Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 và OneCore.
• Con trỏ bám chính xác hơn vào phần văn bản đang thực sự được đọc, với cách chia câu và đoạn câu nhất quán hơn.
• Đã giảm rõ rệt tình trạng con trỏ đi trước, chậm, nhảy bất thường và sự khác biệt giữa các bộ máy giọng nói.
• Vị trí chính xác được giữ tốt hơn sau khi tạm dừng, tiếp tục, tìm kiếm trong tài liệu hoặc đổi bộ máy giọng nói.

Ghi podcast thành các tệp riêng
• Đã thêm tùy chọn “Lưu micrô và âm thanh hệ thống hoặc ứng dụng thành các tệp riêng”.
• Khi ghi đồng thời micrô và một nguồn khác, Sonarpad có thể tạo một tệp chỉ chứa micrô và một tệp thứ hai chứa âm thanh hệ thống, một ứng dụng hoặc các ứng dụng đã chọn.
• Chế độ tách nguồn hỗ trợ cả MP3 và WAV.
• Khi tùy chọn bị tắt, Sonarpad vẫn tạo một tệp trộn duy nhất như trước.
• Các tệp riêng giúp điều chỉnh âm lượng, loại bỏ tiếng ồn và chỉnh sửa podcast, phỏng vấn và hướng dẫn dễ dàng hơn.

Lên lịch ghi radio
• Giờ đây có thể lên lịch ghi radio trước.
• Có thể chọn đài, ngày, giờ và phút bắt đầu cùng thời lượng.
• Hỗ trợ thời lượng tùy chỉnh từ 1 đến 1.440 phút.
• Bản ghi có thể chạy một lần, hằng ngày hoặc hằng tuần.
• Cửa sổ hiển thị rõ hơn các bản ghi đang chạy và đã lên lịch, ngày giờ dự kiến, thời lượng và thời gian còn lại.
• Có thể dùng Windows Task Scheduler để tự động bắt đầu ghi ngay cả khi Sonarpad chưa mở.

Lịch
• Đã thêm lịch đầy đủ, có thể sử dụng hoàn toàn bằng bàn phím.
• Có thể xem ngày trước và ngày sau, nhanh chóng trở về hôm nay và kiểm tra ngày lễ hoặc ngày kỷ niệm.
• Đã thêm vị thánh trong ngày và câu trích dẫn trong ngày, có thể đọc, nghe hoặc sao chép.
• Có thể tạo, sửa, xóa, hoãn và đánh dấu lời nhắc là hoàn tất.
• Thông báo có thể xuất hiện đúng giờ hoặc sớm hơn và hoạt động qua lịch Windows ngay cả khi Sonarpad đã đóng.

Thời tiết
• Đã thêm mục dự báo thời tiết.
• Có thể tìm thành phố và nhanh chóng mở lại các địa điểm đã xem gần đây.
• Có thông tin thời tiết hiện tại, nhiệt độ, mức thấp và cao, độ ẩm, khả năng mưa và dự báo những ngày tiếp theo.
• Có thể chọn độ C, độ F hoặc tự động.

Phim đang chiếu
• Đã thêm mục xem các phim đang chiếu tại rạp và các phim sắp phát hành.
• Có tìm kiếm theo tên, nội dung, ngày phát hành và phát đoạn giới thiệu.

Tổng hợp giọng nói Google
• Đã tích hợp Google TTS để đọc tài liệu và tạo sách nói.
• Đã thêm trình quản lý giọng để xem, lọc theo ngôn ngữ, tải xuống và xóa các giọng không còn cần thiết.
• Có thể điều chỉnh tốc độ, âm lượng và cao độ.
• Cao độ của giọng Google Natural được áp dụng trực tiếp bởi bộ máy để cho kết quả tự nhiên và ổn định hơn.
• Đã cải thiện độ phản hồi và độ tin cậy của Google TTS, với giới hạn thời gian tổng hợp thích ứng theo tốc độ giọng.
• Đã giảm thời gian chờ không cần thiết và cải thiện xử lý lỗi, gián đoạn.

Mục lục EPUB
• Sonarpad giờ nhận diện mục lục được nhúng trong sách EPUB.
• Chương trình thông báo khi có mục lục và có thể mở từ menu Xem.
• Chương và mục con được hiển thị theo cấu trúc phân cấp.
• Nhấn Enter để chuyển ngay đến vị trí đã chọn.

Tin tức và nguồn RSS
• Mục Tin tức được mở rộng với các công cụ tìm kiếm và sắp xếp mới.
• Đã thêm lựa chọn ngôn ngữ tin tức.
• Có thể tìm trong nguồn RSS và đọc tin của thành phố mình.
• Có thể duyệt, thêm vào bộ sưu tập cá nhân và gửi nguồn RSS cho cộng đồng Sonarpad.

Ghi podcast
• Có thể ghi chỉ micrô, toàn bộ âm thanh hệ thống, một ứng dụng, nhiều ứng dụng đã chọn hoặc micrô và ứng dụng cùng lúc.
• Có thể chọn thiết bị micrô và nguồn âm thanh, điều chỉnh âm lượng riêng và theo dõi mức âm lượng theo thời gian thực.
• Đã thêm tạm dừng và tiếp tục, đầu ra MP3 hoặc WAV, chọn bitrate MP3 và thư mục đích.
• Có thể giữ máy tính hoạt động trong khi ghi.

Radio
• Mục Radio đã được tổ chức lại đáng kể.
• Có thể tìm đài theo tên hoặc văn bản tự do, ngôn ngữ, quốc gia, thành phố, thể loại nhạc hoặc danh mục.
• Đã cải thiện quản lý yêu thích và thêm cách xóa nhanh toàn bộ bộ lọc.
• Có thể gửi đài cho cộng đồng Sonarpad.
• Đã thêm ghi trực tiếp, chế độ “Ghi và phát”, danh sách bản ghi cùng khả năng quản lý và xóa.
• Bản ghi radio được lưu trong thư mục riêng bên trong thư mục ghi âm chính.

Phát đa phương tiện
• Độ ổn định của trình phát đa phương tiện đã được cải thiện đáng kể.
• Đã sửa lỗi có thể làm mpv bị treo và cải thiện giao tiếp với trình phát.
• Cải thiện việc mở các loại tệp đa phương tiện khác nhau.
• Sonarpad giờ ghi nhớ mức âm lượng đã dùng.
• Cải thiện xử lý luồng và bản ghi.
• Sửa việc mở tệp từ Windows bằng nhấp đúp hoặc “Mở bằng”.

Tài liệu PDF
• Đã thêm khả năng nhận diện các trường biểu mẫu trong PDF.
• Sonarpad có thể tìm trường có thể điền, trình bày chúng dưới dạng văn bản dễ tiếp cận, cho phép sửa và lưu dữ liệu vào PDF.
• Đã sửa cách tính vị trí con trỏ khi đọc, đặc biệt với ký tự nhiều byte và cấu trúc phức tạp.

Khả năng tiếp cận và bàn phím
• Cải thiện các lệnh chỉnh sửa thông thường trong toàn bộ chương trình.
• Sao chép, cắt, dán, chọn tất cả, hoàn tác và làm lại được gửi đúng đến trường đang có tiêu điểm, kể cả cửa sổ phụ và hộp thoại.
• Đã sửa lỗi cập nhật màn hình chữ nổi.
• Cải thiện quản lý tiêu điểm và sửa lựa chọn ngôn ngữ trong Wikipedia.
• Đã thêm tùy chọn nhóm các chức năng trong menu Công cụ theo danh mục.
• Đã thêm hành động có thể cấu hình để nhanh chóng mở Lịch, Thời tiết và Phim đang chiếu.

Sách nói
• Cải thiện việc tạo sách nói khi hộp thoại hoặc cửa sổ phương thức đang mở.
• Quản lý tiến trình đáng tin cậy hơn và bỏ qua các cập nhật âm thanh đã cũ.
• Google TTS cũng có thể dùng để tạo sách nói với điều khiển tốc độ, âm lượng và cao độ.

Trí tuệ nhân tạo
• Đã cập nhật mô hình Gemini mặc định thành `gemini-3.5-flash`.

Sửa lỗi chung
• Sửa một số lỗi treo khi phát bằng mpv.
• Sửa việc mở một số tệp âm thanh và video.
• Cải thiện xử lý lệnh gửi đến trình phát.
• Sửa việc khôi phục con trỏ trong khi đọc.
• Cải thiện độ ổn định khi tạo sách nói.
• Cải thiện tổng thể việc xử lý đa phương tiện, RSS, radio và EPUB.

Phiên bản 0.7.1 – 2026-05-13

Tính năng mới và cải tiến
• Đã tạo trang web chính thức sonarpad.com, một điểm tham khảo mới để theo dõi những tin tức mới nhất, tải xuống phiên bản mới nhất của chương trình, đọc bình luận của khách truy cập và, trong tương lai, nghe tất cả podcast của Sonarpad. Mục “Truy cập sonarpad.com” cũng đã được thêm vào menu Trợ giúp, để mở nhanh trang web chính thức.
• Đã sửa lỗi khiến các tệp có dấu hoặc ký tự đặc biệt gây lỗi khi bắt đầu phiên âm bằng giọng nói.
• Từ bây giờ, trong menu Xem, các mục như Tự động xuống dòng và Hiển thị video trong khi phát sẽ luôn hiển thị đúng trạng thái, bật hoặc tắt.
• Cải thiện tìm kiếm YouTube, cho phép quay lại trang hoặc màn hình trước bằng phím Esc.
• Thêm kiểm tra sơ bộ để xác minh video có thể phát được hay không. Việc phát cũng được cải thiện: Sonarpad giờ có thể phát cả video hoặc danh sách phát được đánh dấu là mix, vốn trước đây không phát được.
• Cải thiện quản lý dấu trang tự động. Trước đây, nếu tùy chọn Dấu trang tự động được bật rồi tắt, các dấu trang đó vẫn còn; giờ chương trình sẽ bỏ qua chúng đúng cách cho đến khi tùy chọn được bật lại. Ngoài ra, khi phát đến cuối tệp media, dấu trang sẽ tự động bị xóa.
• Cải thiện quản lý thẻ khi hộp thoại đang bật. Sonarpad giờ xử lý đúng cả hai chức năng, cho phép chèn thẻ ngay cả khi tùy chọn hộp thoại đang bật.
• Cải thiện cài đặt giọng nói bằng cách tách rõ từng công cụ, giúp việc điều chỉnh chính xác hơn. Hồ sơ giọng nói giờ lưu đúng cài đặt cho từng công cụ riêng lẻ: Edge, Sapi5 và Sapi4.
• Thêm thẻ để chèn khoảng dừng, trực tiếp từ tùy chọn hoặc từ bảng giọng nói bằng cách nhấn Tab từ trình soạn thảo. Các lựa chọn gồm: 250 ms, 500 ms, 1 giây, 2 giây hoặc thời lượng tùy chỉnh.
• Sửa hành vi khi phát video YouTube và bắt đầu phiên âm. Giờ khi quay lại bằng Alt+Tab, tiêu điểm sẽ nằm đúng trên nút Hủy của phiên âm đang chạy.
• Từ nay, bản phiên âm sẽ được tự động lưu khi quá trình hoàn tất.
• Cải thiện nhập từ Wikipedia. Có thể chọn chỉ đọc một phần rồi từ bài viết nhấn Esc để quay lại tìm kiếm, hoặc nhập toàn bộ bài viết. Cũng có thể chọn ngôn ngữ Wikipedia cần tra cứu.
• Thêm mục radio từ khắp thế giới, nơi có thể tìm radio theo quốc gia, ngôn ngữ và thể loại. Cũng có thể thêm radio địa phương vào cơ sở dữ liệu Sonarpad, để người dùng khác cũng có thể nghe. Radio cũng có thể được thêm vào mục yêu thích.
• Thêm mục tuyến đường để tính đường đi bằng cách chọn phương tiện: đi bộ, xe đạp, ô tô hoặc xe lăn. Có thể chọn tuyến ngắn nhất hoặc nhanh nhất và có hiển thị các đô thị đi qua hay không. Sau khi nhập tuyến đường, cũng có thể lưu bản đồ trực quan từ menu Tệp, Lưu hình ảnh.
• Thêm mục In trong menu Tệp. Sonarpad sẽ in tệp TXT bằng chính chương trình, và dùng chương trình liên kết cho các tệp khác như DOCX, PDF và tương tự, để giữ bố cục gốc tốt nhất có thể.
• Tích hợp vào Sonarpad dịch vụ dịch cho từng tài liệu, truy cập từ menu ngữ cảnh của trình soạn thảo. Người dùng có thể dùng miễn phí DeepL và Google Translate mà không cần nhập khóa API; nếu nhập khóa API Gemini, có thể dịch bằng Gemini.
• Trong menu dịch, người dùng có thể chọn ngôn ngữ đích. Menu sẽ tự sắp xếp lại: nếu người dùng chọn tiếng Anh trước, rồi tiếng Pháp, rồi tiếng Ý, ba tùy chọn này sẽ xuất hiện ở đầu menu ngôn ngữ.
• Nếu người dùng nhập khóa API Gemini của mình, họ cũng có thể dùng chức năng Tóm tắt văn bản, luôn có trong menu ngữ cảnh, để tóm tắt bất kỳ bài viết nào.
• Thêm vào menu Phát, xuất hiện khi đang phát tệp media, một menu để chia media hiện tại. Chức năng này hoạt động với MP3, MP4 và các định dạng khác, chia theo số phần hoặc theo thời lượng của mỗi phần.

Phiên bản 0.7.0 – 2026-04-25

Bản mới
• Đã thêm hỗ trợ trình phát mpv cho phát trực tuyến. Video từ YouTube và các trang được hỗ trợ giờ phát ngay lập tức; nếu người dùng muốn lưu lại, chúng sẽ được tải xuống như trước. Khi phiên âm nội dung streaming, nội dung sẽ được tải xuống trước rồi mới xử lý. Trình phát mpv cũng được dùng để mở video cục bộ và xử lý phụ đề, giúp cải thiện khả năng tương thích với nhiều định dạng.
• Đã cải thiện tính năng ghi podcast từ âm thanh hệ thống: giờ đây bạn có thể chọn ghi toàn bộ âm thanh hệ thống, một ứng dụng hoặc nhiều ứng dụng cùng lúc. Tùy chọn này được tích hợp vào chế độ ghi bình thường, vì vậy vẫn có thể bật hoặc tắt micro riêng biệt.
• Đã thêm ngôn ngữ Hindi. Giao diện đã được dịch, bổ sung RSS, nhật ký thay đổi và hướng dẫn Sonarpad.
• Đã thêm tùy chọn trong tab Trình soạn thảo để luôn đưa con trỏ về đầu dòng khi dùng phím mũi tên lên và xuống.
• Đã thêm tùy chọn trong menu "Chuyển đổi âm thanh" để chuyển đổi âm thanh sang M4B.

Bản sửa lỗi
• Trong phần bình luận YouTube mở từ "Phat am thanh tu streaming...", Sonarpad giờ chỉ tải trước 50 bình luận gốc đầu tiên, luôn kèm toàn bộ phản hồi của các bình luận đó, và thêm ở cuối một mục để tải tất cả bình luận khi cần.
• Dấu trang giờ được hiển thị và xử lý theo vị trí của chúng trong cả tài liệu văn bản lẫn tệp đa phương tiện, thay vì theo thứ tự tạo. Nếu đã có dấu trang ở cùng vị trí, nó sẽ không được thêm lại nữa.
• Đã thêm một tùy chọn trong menu Dấu trang, khi được bật, cho phép quản lý dấu trang tự động. Khi phát một tệp cục bộ hoặc tệp phát trực tuyến rồi đóng lại, Sonarpad sẽ tự động đặt dấu trang dựa trên vị trí đã phát tới và khi mở lại tệp, chương trình sẽ tiếp tục từ vị trí đó. Điều tương tự cũng áp dụng cho các tệp văn bản: nếu mở một văn bản và di chuyển con trỏ, Sonarpad sẽ ghi nhớ vị trí đó khi đóng tệp; nếu bắt đầu đọc, câu cuối cùng đã đọc sẽ được lưu lại và việc đọc sẽ tiếp tục chính xác từ điểm đó.
• Đã thêm một mục trong menu Xem để hiển thị phần kết xuất video cho các tệp cục bộ hoặc tệp phát trực tuyến. Nội dung video được hiển thị trong một cửa sổ phóng to, trong đó tất cả các điều khiển đều được ẩn, trừ khi nhấn phím Alt hoặc di chuyển chuột lên phía trên của cửa sổ. Bằng cách này, người dùng khiếm thị một phần sẽ có nội dung lớn hơn và dễ sử dụng hơn.

Phiên bản 0.6.9 – 2026-04-08

Bản sửa lỗi
• F5 trước đây luôn bắt đầu đọc từ đầu tài liệu. Giờ lỗi này đã được sửa và việc đọc bắt đầu từ vị trí con trỏ hiện tại, đồng thời vẫn giữ `Shift+F5` và `Ctrl+F5` để chuyển đến câu trước hoặc câu tiếp theo.
• Sau khi dùng Đi đến dòng, nhấn Esc có thể làm mất focus khỏi Sonarpad. Giờ focus sẽ quay lại trình soạn thảo đúng cách.
• Tùy chọn `Tự động xuống dòng` nay được áp dụng ngay cả với tài liệu đang mở, không còn phải mở lại tệp mới thấy thay đổi.

Phiên bản 0.6.8 – 2026-04-07

Có gì mới
• Đã thêm một mục mới trong menu Phát để chép lời bất kỳ tệp âm thanh hoặc video nào bằng Whisper. Trong Tùy chọn có một phần mới tên là “AI và Chuyển lời”, nơi bạn có thể chọn mô hình, bật hỗ trợ CUDA tùy chọn cho card đồ họa NVIDIA, giữ nguyên ngôn ngữ gốc và bật hoặc tắt dấu thời gian.
• Đã thêm vào menu Phát hành động mới `Chuyển đổi thư mục hiện tại`, cho phép chuyển đổi tất cả các tệp âm thanh được hỗ trợ trong thư mục của media đang mở và gộp chúng thành một tài liệu duy nhất, với cửa sổ tiến trình riêng, thông tin tệp hiện tại và khả năng hủy. Cũng có thể gọi bằng phím tắt `Alt+Shift+C`.
• Đã thêm khả năng dùng đọc chính tả bằng giọng nói ngoại tuyến, với cách hoạt động giống như chép lời âm thanh. Mặc định, nhấn `Ctrl+Shift+Space` để bắt đầu đọc chính tả và nhấn lại đúng phím tắt đó để kết thúc; có thể tùy chỉnh phím tắt trong phần Tùy chọn. Từ lần kích hoạt thứ hai trở đi, việc đọc chính tả sẽ nhanh hơn vì bộ máy vẫn sẵn sàng trong bộ nhớ; trên các PC có dưới 4 GB RAM, việc nạp sẵn và tái sử dụng này sẽ tự động bị tắt.
• Đã thêm một tùy chọn mới trong phần trình soạn thảo, mặc định tắt, cho phép `Esc` đóng cửa sổ trình soạn thảo.
• Tìm kiếm podcast giờ mặc định dùng `iTunes + Spreaker`, với bộ lọc loại bỏ kết quả trùng lặp khi cùng một podcast xuất hiện trên cả hai nền tảng.
• Đã cải thiện tìm kiếm và duyệt podcast Apple: tìm kiếm podcast, duyệt theo danh mục và top podcast theo danh mục giờ dùng quốc gia thư mục podcast đã chọn. Trong Tùy chọn > RSS / Podcast, có thể để `Tự động` để dùng quốc gia hệ thống hoặc tự chọn một quốc gia khác.
• Đã tăng giới hạn kết quả cho các danh mục podcast Apple. Khi mở lần đầu, Sonarpad vẫn tải 50 kết quả đầu tiên như trước; nếu bạn chọn `Tải thêm kết quả`, Sonarpad sẽ tải tối đa 200 kết quả tổng cộng (giới hạn của Apple) và cho phép duyệt các trang tiếp theo trong khi vẫn giữ trải nghiệm mượt mà.
• Sonarpad hiện cũng đã có bản cho Mac, dù hiện chỉ hỗ trợ một phần chức năng. Liên kết dự án: https://github.com/Ambro86/Sonarpad-Mac

Cải tiến
• Đã thêm hơn 50 quốc gia có thể chọn cho thư mục podcast, giúp người dùng chọn được nhiều danh mục quốc gia hơn.
• "Phat am thanh tu streaming..." gio cung cho phep tim kiem tren YouTube bang bat ky chuoi van ban nao, hoac dan lien ket cua mot kenh hoac playlist YouTube de hien thi cac ket qua cua no.
• Đã cải thiện cách hiển thị kết quả trong "Phat am thanh tu streaming...": các mục YouTube giờ bao gồm tiêu đề, thời lượng, kênh và lượt xem theo định dạng rõ ràng hơn.
• "Phat am thanh tu streaming..." giờ cũng hỗ trợ bình luận YouTube: có thể mở từ menu ngữ cảnh, đọc các câu trả lời và mở rộng các luồng bình luận bằng phím Mũi tên phải.
• Đã thêm mục yêu thích YouTube cho kênh và danh sách phát trong "Phat am thanh tu streaming...": có thể thêm từ kết quả bằng menu ngữ cảnh, mở trực tiếp từ danh sách Yêu thích truy cập bằng Tab ngay sau trường URL/truy vấn YouTube và xóa sau đó cũng từ chính danh sách đó bằng menu ngữ cảnh. Trong kết quả tìm kiếm YouTube, menu ngữ cảnh chỉ khả dụng cho kênh và danh sách phát.
• "Phat am thanh tu streaming..." giờ có thể yêu cầu thông tin đăng nhập khi một trang cần đăng nhập. Người dùng có thể nhập, lưu cho trang đó và quản lý các thông tin đã lưu sau này trong Tùy chọn > Âm thanh.
• Đã cải thiện focus khi dùng "Phat am thanh tu streaming...", để cửa sổ tiến trình ổn định hơn trong lúc tải xuống và chuyển đổi.
• Đã thêm hai thao tác đọc mới trong menu Giọng nói: `Câu trước` và `Câu tiếp theo`, với phím tắt có thể cấu hình để nhảy trong khi đọc văn bản.
• Phím tắt mặc định của `Chạy tệp bằng trình thông dịch` giờ là `Ctrl+Shift+F5`, để `Shift+F5` có thể được dùng mặc định cho `Câu trước`.
• Đã thêm quản lý hồ sơ giọng nói trong Tùy chọn > Giọng nói: có thể thêm, đổi tên và xóa hồ sơ.
• Đã mở rộng trong Tùy chọn > Âm thanh các lựa chọn cho khoảng tua lùi khi phát, với các giá trị mới từ 1 giây đến 2 giờ.
• Đã thêm bản dịch tiếng Nga nhờ Dmitriy.
• Đã thêm trong Tùy chọn > Âm thanh một lựa chọn mới cho định dạng tên phần của sách nói: `Tiêu đề + số`, `Chỉ số` hoặc `Số + tiêu đề`.
• Đã thêm hành động trong menu ngữ cảnh bài RSS để thêm bài viết vào mục yêu thích.
• Nguồn RSS "Yêu thích" có thể bị xóa và sẽ được tạo lại tự động khi thêm bài viết mới vào mục yêu thích.
• Đã thêm phím tắt RSS để di chuyển nguồn lên/xuống: `Ctrl+Shift+Mũi tên lên` và `Ctrl+Shift+Mũi tên xuống`.
• Đã cải thiện cửa sổ RSS với phần xem trước bài viết tích hợp, giúp có thể xem ngay nội dung tại đó và chuyển nhanh tới bằng phím Tab trước khi mở toàn bộ bài viết trong trình soạn thảo.
• Đã thêm trong RSS một mục rõ ràng “Tải thêm tin tức” ở cuối nguồn khi còn bài khác; nhấn Enter sẽ tải khối tiếp theo và đưa focus tới bài viết mới đầu tiên.
• Trong từ điển giọng nói, khi thêm hoặc sửa một mục thay thế, giờ có thêm ô «Phân biệt chữ hoa/thường» để quyết định mỗi phép thay thế có phân biệt hay bỏ qua chữ hoa chữ thường.
Sửa lỗi
• "Phat am thanh tu streaming..." giờ tôn trọng giới hạn bộ nhớ đệm podcast đã đặt trong Tùy chọn, và giới hạn này cũng được áp dụng cho việc phát audio description.
• Đã sửa chức năng nhập từ Wikipedia: trên một số trang, các đoạn trích dẫn trong bài không được nhập đúng.
• Đã cải thiện bộ phân tích trang web: trên một số trang WordPress, các mục danh sách và một số tiêu đề phần không được đưa vào.
• Khi dùng “Đi đến dòng”, ô nhập giờ sẽ được điền sẵn bằng dòng hiện tại.
• Đã sửa xuất OPML cho podcast và RSS, vì vậy các tệp xuất ra giờ đã được iTunes chấp nhận.
• Đã sửa phần chép lời tệp phương tiện: giờ khi đóng tài liệu được tạo bằng Alt+F4, Sonarpad sẽ hỏi có muốn lưu hay không và đề xuất đúng tên tệp dựa trên tên tệp đã được chép lời thay vì dòng đầu tiên của văn bản.
• Đã thêm thông báo xác nhận được bản địa hóa cho việc nhập và xuất OPML podcast và nguồn RSS đúng cách.
• Đã sửa lỗi trong "Phat am thanh tu streaming..." khiến chương trình có thể trông như bị treo khi nhập chuỗi tìm kiếm và chọn một kênh YouTube từ kết quả, thay vì mở danh sách video của kênh đó.
• Đã sửa lỗi khiến danh sách tệp đang mở hiển thị trong menu Trợ giúp thay vì menu Cửa sổ.
• Đã sửa một trường hợp biên khi phát trực tuyến: phát âm thanh có thể bắt đầu nhưng cửa sổ “Đang tải streaming” vẫn mở khi tệp đã tải xuống đã đúng định dạng đích.
• Đã sửa hành vi chuyển đổi với streaming MP3: khi luồng đã là MP3 và người dùng chọn bitrate MP3 cụ thể (ví dụ 128 kbps), Sonarpad giờ sẽ mã hóa lại theo bitrate đã chọn thay vì bỏ qua bước chuyển đổi.
• Đã sửa phím tắt `Alt+Shift+L`: giờ đây phím này mở đúng danh sách chương trong khi phát.
• Đã sửa phím tắt `Alt+Shift+T`: giờ đây phím này khởi động đúng chức năng “Chép lời âm thanh hiện tại” thay vì mở menu Công cụ.
• Nếu đang có âm thanh được phát, khi bắt đầu chép lời Sonarpad giờ sẽ tự động tạm dừng âm thanh đó trước khi bắt đầu.
• Đã sửa lỗi khiến khi nhập một bài viết từ Wikipedia, quá trình nhập có thể thành công nhưng phần văn bản bài viết lại không hiển thị trên màn hình.
• Đã bổ sung hỗ trợ chương podcast nhúng trong tệp media cục bộ (ví dụ metadata chương của MP3): khi feed/URL không cung cấp chương, Sonarpad sẽ nạp chương từ tệp đã tải ở chế độ nền, giúp phát bắt đầu ngay và áp dụng chương ngay khi sẵn sàng.
• Đã sửa việc nạp chương cho các tập podcast đã tải xuống rồi mở như tệp media cục bộ thông thường: các chương nhúng giờ cũng khả dụng trong trường hợp này, không chỉ khi bắt đầu phát từ cửa sổ Podcast.
• Đã sửa bước hoàn tất cuối cùng của sách nói MP3 với SAPI4 và SAPI5: tệp đầu ra cuối cùng giờ được hoàn tất đúng cách, tránh các tệp không đầy đủ hoặc kém ổn định sau các lần xuất dài.
• Đã thêm thanh tiến trình rõ ràng cho giai đoạn hoàn tất trong mọi chế độ tạo sách nói: sau giai đoạn tạo, Sonarpad giờ thông báo và hiển thị riêng giai đoạn hoàn tất với tiến trình nhìn thấy được.
• Đã sửa lỗi giọng hội thoại: các tham số tốc độ/cao độ/âm lượng của giọng hội thoại thứ nhất và thứ hai giờ được áp dụng đúng trong quá trình tổng hợp giọng nói.
• Đã cải thiện nhận diện mã hóa cho tệp `.txt` tiếng Nhật: thêm fallback Shift_JIS/CP932 an toàn cho các trường hợp mojibake, đồng thời giữ nguyên hành vi hiện có với UTF/diacritics/tiếng Trung.
• Tái cấu trúc an toàn nội bộ: chuyển sang triển khai safe ở những nơi có thể và giảm mạnh số dòng mã unsafe.

Phiên bản 0.6.7 – 2026-03-02
Cải tiến
• Ora il programma riesce a gestire Sostituisci tutto in modo massivo su file grandi con un gran numero di sostituzioni.
• Đã cập nhật bản dịch tiếng Ba Lan nhờ DJ Graco.
• Đã thêm bản dịch tiếng Litva.
• Đã thêm bản dịch tiếng Trung.
• Từ bây giờ, các bản dựng beta thường xuyên sẽ được phát hành trong mục Releases của dự án, để người dùng có thể thử nghiệm các thay đổi mới trước bản phát hành ổn định tiếp theo.
• Đã thêm phím tắt `Ctrl+.` để chèn ký tự dấu ba chấm (…).
• Đã cải thiện hỗ trợ chương podcast: điều hướng chương hiện ổn định hơn, kể cả với các tập phát trực tiếp/streaming không có chương nhúng trong tệp MP3, bằng cách dùng metadata chương từ feed/URL làm fallback khi có sẵn. Đã thêm phím tắt `Ctrl+Alt+PageUp` (chương trước) và `Ctrl+Alt+PageDown` (chương tiếp theo).
• Đã tổ chức lại thư mục đầu ra về `Documents\\Sonarpad`: tệp giờ được lưu vào các thư mục con riêng `audiobooks`, `documents`, `recordings` và `media`, kèm tự động chuyển dữ liệu từ đường dẫn cũ.
• Đã cải thiện hỗ trợ cho tệp văn bản rất lớn (kể cả 60 MB): mở tệp và điều hướng theo từng dòng mượt hơn, đặc biệt khi dùng trình đọc màn hình.
• Đã cập nhật hướng dẫn cho tất cả ngôn ngữ và làm mới tài nguyên bản địa hóa trên toàn bộ ứng dụng, bao gồm nội dung quyên góp và bản dịch trình cài đặt NSIS (thêm chuỗi cài đặt mới cho tiếng Trung giản thể và tiếng Litva, đồng thời hoàn thiện bản dịch tiếng Ukraina của setup).
• Da bo sung ho tro proxy mang toan cuc (HTTP/HTTPS va SOCKS5/SOCKS5H) cho cac tinh nang truc tuyen, kem kiem tra khi luu Tuy chon: proxy khong hop le se duoc canh bao va xoa tu dong.
• Da them tinh nang moi trong Cong cu: "Phat am thanh tu streaming...", cho phep dan URL (YouTube hoac lien ket media truc tiep), chon dinh dang dau ra va ho so chat luong/bitrate (bao gom chat luong/bitrate goc cho MP3 va MP4) va phat ngay trong trinh phat cua Sonarpad.
• Da them ho tro phim da phuong tien Play/Pause cua he thong (tai nghe/ban phim): nay co the dieu khien ca phat media va tam dung/tiep tuc doc van ban (uu tien trinh phat media khi ca hai dang hoat dong).
• Da them muc moi trong Tep > Tep gan day: "Xoa tep gan day" de xoa nhanh danh sach tai lieu vua mo.
• Đã mở rộng tùy chọn bitrate trong Chuyển đổi âm thanh và cài đặt ghi podcast: thêm mức thấp hơn (64/96 kbps) và nâng MP3 lên tối đa 320 kbps, đồng bộ cả kiểm tra hợp lệ và xử lý bộ mã hóa.
• Đã mở rộng tùy chọn chia sách nói theo thời lượng lên đến 60 phút.
• Đã cải thiện chia sách nói theo số phần: người dùng có thể nhập thủ công số phần, với kiểm tra hợp lệ từ 1 đến 100.
• Đã thêm chế độ mới Xem > Chỉ đọc để ngăn chỉnh sửa nhầm văn bản, đồng thời vẫn giữ khả năng đọc và điều hướng đầy đủ tài liệu.
• Đã thêm thanh tiến trình có thể truy cập trong quá trình cập nhật chương trình, giúp trình đọc màn hình theo dõi tiến độ tải xuống theo thời gian thực.
• Đã thêm thanh trạng thái mới dạng yên lặng ở cửa sổ chính với số ký tự, số từ và vị trí dòng/cột (ví dụ: "Ký tự (kể cả khoảng trắng): 11. | Từ: 2. | Ln 1, Col 12"), không làm ảnh hưởng tiêu điểm của NVDA.
• Đã thêm tùy chọn mới trong menu Xem cho tự động xuống dòng, giúp bật/tắt nhanh chế độ xuống dòng mà không cần mở Tùy chọn.
• Đã thêm trong Chỉnh sửa > Văn bản các thao tác tăng/giảm thụt lề, với phím tắt Ctrl+Shift+. (thụt vào) và Ctrl+Shift+, (thụt ra), vì khi bật “Hiển thị giọng nói trong trình soạn thảo” thì phím Tab được dành cho việc điều hướng bảng giọng nói.
• Đã thêm hiển thị ngày/giờ theo ngôn ngữ cho bài RSS và tập podcast, với định dạng tự điều chỉnh theo ngôn ngữ giao diện.
• Đã thêm thao tác mới trong menu ngữ cảnh RSS để chia sẻ bài đã chọn qua email.
• Đã thêm tùy chọn xác nhận xóa chi tiết trong Tùy chọn > RSS và podcast: với RSS (nguồn/bài/cả hai/không) và với podcast (podcast/tập/cả hai/không).
• Đã thêm sao chép nhanh RSS có thể cấu hình bằng Ctrl+C (Tùy chọn > RSS và podcast): sao chép tiêu đề, URL, nội dung bài viết hoặc tất cả.
• Đã hợp nhất luồng RSS: “Thêm nguồn” giờ chấp nhận cả URL feed và từ khóa (tự động tạo feed Google News), không cần chức năng tìm kiếm riêng.
• Khi nhấn Ctrl+A, chương trình giờ sẽ thông báo hoàn tất thao tác để phản hồi rõ ràng hơn cho trình đọc màn hình.
• Đã thêm Shift+F3 cho "Tìm trước đó" trong menu Chỉnh sửa, bổ sung cho F3 "Tìm tiếp theo".
• Đã cải thiện thông báo thay thế với dạng số ít/số nhiều chính xác (ví dụ: “1 mục đã thay thế” và “2 mục đã thay thế”).
• Đã thêm trong cửa sổ từ điển tùy chọn chọn ngôn ngữ tra cứu, mặc định là Auto (ngôn ngữ giao diện) và có thể chọn thủ công.
• Đã thêm tab Phím tắt mới trong Tùy chọn để tùy chỉnh tổ hợp phím, kèm phát hiện xung đột và cảnh báo khi một phím tắt đã được gán cho hành động khác.
• Đã thêm hỗ trợ ban đầu cho tham số dòng lệnh: `-h`/`--help` hiển thị trợ giúp nhanh và `--version` hiển thị phiên bản chương trình.
• Đã làm rõ hơn cách chỉnh tay tốc độ và cao độ: các ô chỉnh tay giờ dùng thang lấy 100 làm mốc, trong đó 100 tương ứng giá trị bình thường.
• Đã cải thiện chọn giọng Microsoft trong cả Tùy chọn > Giọng nói và bảng giọng nói của trình soạn thảo: thêm combobox ngôn ngữ đã bản địa hóa để lọc giọng theo ngôn ngữ, đồng thời vẫn giữ chế độ “chỉ giọng đa ngôn ngữ” là một danh sách duy nhất không chia theo ngôn ngữ (ẩn combobox ngôn ngữ khi chế độ này bật).
• Đã thêm cấu hình giọng cho hội thoại trong Tùy chọn > Giọng nói với điều hướng đầy đủ bằng Tab, dùng cùng mô hình giọng của giao diện chính (engine, bộ lọc ngôn ngữ Edge, giọng và tốc độ/cao độ/âm lượng có nhãn); thêm giọng hội thoại thứ hai tùy chọn với cùng nhóm điều khiển (engine, bộ lọc ngôn ngữ Edge, giọng, tốc độ/cao độ/âm lượng) để luân phiên hội thoại; quy tắc hội thoại được lưu trong cấu hình `.ini`, không sửa đổi văn bản tài liệu.
• Đã cải thiện nhãn Hoàn tác: mục Chỉnh sửa > Hoàn tác giờ hiển thị hành động sẽ được hoàn tác (ví dụ chỉnh sửa văn bản, thêm/bỏ comment dòng hoặc chèn thẻ giọng nói), đồng thời vẫn bị vô hiệu khi không có gì để hoàn tác.
Sửa lỗi
• Đã sửa hỗ trợ mở tệp RTF: tệp `.rtf` giờ được trích xuất và hiển thị thành văn bản dễ đọc, không còn hiển thị mã RTF thô (ví dụ `{\\rtf1...}`).
• Đã sửa mở tệp văn bản tiếng Trung mã hóa GB18030/GBK: Sonarpad giờ phát hiện và giải mã đúng, tránh hiện tượng chữ lỗi (mojibake).
• Đã cải thiện tạo sách nói M4B với metadata và marker chương; đã sửa lỗi "chipmunk" (giọng quá cao/quá nhanh) trong tệp M4B tạo ra.
• Đã sửa giao diện bitrate trong cửa sổ lưu sách nói: đã bỏ các nhãn hardcoded bằng tiếng Ý và thêm tùy chọn 64 kbps vào danh sách bitrate có thể chọn.
• Đã sửa "Lưu tất cả" (Ctrl+Shift+S): giờ đây tất cả tài liệu đang mở đã chỉnh sửa đều được phát hiện ổn định (kể cả tab mới/chưa lưu), và Lưu tất cả sẽ lưu đúng từng tài liệu, mở "Lưu thành" khi cần.
• Đã sửa thứ tự bài RSS từ Google News: khi có ngày xuất bản, bài viết giờ được hiển thị từ mới nhất đến cũ nhất.
• Đã sửa gán nhãn cho NVDA trong cửa sổ từ điển: ô tìm kiếm và combobox ngôn ngữ giờ đọc đúng nhãn.
• Đã sửa điều hướng bàn phím trong cửa sổ Thuộc tính RSS/Podcast: Tab/Shift+Tab giờ đi tới nút OK, Enter kích hoạt OK, Esc đóng an toàn và tiêu điểm quay lại đúng danh sách RSS/Podcast.
• Đã sửa lịch sử hoàn tác trong RSS/Podcast: Ctrl+Z giờ hỗ trợ hoàn tác nhiều cấp cho thao tác xóa (bài/tập và nguồn), không chỉ thao tác gần nhất.
• Đã cải thiện thông báo khi xóa trong RSS/Podcast với thông điệp rõ ràng (đã xóa RSS, đã xóa bài RSS, đã xóa tập podcast).
• Đã cải thiện xử lý tiêu điểm sau khi xóa/hoàn tác trong RSS/Podcast: với RSS, mục nguồn đầu tiên được chọn lại ổn định khi cần, đồng thời giảm lặp thông báo của trình đọc màn hình trong quá trình chọn lại có độ trễ.

Phiên bản 0.6.6 – 2026-02-13
Cải tiến
• Đã thêm "Định dạng tự động cho TTS" trong menu Chỉnh sửa để chuẩn bị nhanh văn bản cho đọc giọng nói (xóa markdown/dấu ngoặc kép và ghép lại các dòng bị ngắt).
• Đã cải thiện chèn thẻ giọng nói: khi có văn bản được chọn, thẻ giờ được áp dụng đúng cho cả một dòng đơn và vùng chọn nhiều dòng.
• Đã thêm tùy chọn trong cài đặt Âm thanh để chọn thư mục mặc định lưu sách nói (mặc định: Documents\\Sonarpad Audiobooks).
• Trong hộp thoại lưu sách nói, khi bật chế độ chia phần, đã thêm tùy chọn mới (bật mặc định) để tạo thư mục con riêng cho các phần được tạo ra.
• Xuất sách nói giờ lưu MP3 stereo với bitrate do người dùng chọn cho giọng Edge, SAPI5 và SAPI4.
• Đã thêm hỗ trợ giọng SAPI5 32-bit qua bridge, để dùng cả các giọng chỉ có trong engine 32-bit.
• Đã tổ chức lại các tính năng giọng nói vào menu riêng "Giọng nói và âm thanh" và thêm/làm rõ mục "Chuyển đổi âm thanh", dùng để chuyển đổi mọi tệp đa phương tiện được hỗ trợ sang MP3, AAC, OGG, Opus, FLAC, WAV và AIFF.
• Đã thêm khả năng xóa từng bài RSS và từng tập podcast riêng lẻ (phím Delete + menu ngữ cảnh có xác nhận), không cần xóa toàn bộ nguồn RSS/podcast, kèm hoàn tác lần xóa gần nhất (bài/tập đơn lẻ hoặc toàn bộ nguồn RSS/podcast).
• Đã thêm chức năng xuất nguồn RSS sang OPML trong cửa sổ RSS, giúp lưu và nhập lại các nguồn hiện tại một cách dễ dàng.
• Đã thêm tính năng "Tìm RSS theo từ khóa" trong cửa sổ RSS: khi nhập từ khóa, Sonarpad sẽ tự động tạo URL RSS Google News và mở hộp thoại thêm nguồn với thông tin đã điền sẵn, giúp tạo feed theo chủ đề chỉ trong một bước.
• Đã thêm bản dịch tiếng Serbia nhờ Mila Kuran.
• Đã thêm bản dịch tiếng Ukraina nhờ Ivan Shtefuriak.
• Đã thêm mở nhiều tệp media cùng lúc: khi mở nhiều tệp sẽ tạo hàng đợi phát thay vì thay thế tệp hiện tại.
• Đã thêm phím tắt tua biến thiên khi phát: với mức cơ bản 1 phút, Trái/Phải tua 60 giây, Shift+Trái/Phải tua 20 giây, và Ctrl+Trái/Phải tua 3 phút.
• Đã thêm phím tắt chuyển bài trước/sau trong trình phát: Ctrl+PageUp và Ctrl+PageDown.
• Đã thêm mục "Đặt lại âm lượng" và gom các thao tác đặt lại vào submenu riêng "Đặt lại" trong menu Phát lại, cùng với "Đặt lại tốc độ" và "Đặt lại cao độ".
• Cải tiến trình cài đặt: setup.exe giờ cho phép chọn giữa liên kết tất cả kiểu tệp được hỗ trợ hoặc tự chọn từng phần mở rộng; MSI cũng hỗ trợ chọn theo từng phần mở rộng trong cây tính năng (mặc định giữ nguyên: bật tất cả).
• Đã thêm menu mới "Cửa sổ" với mục "Tài liệu đang mở..." để chuyển nhanh đến bất kỳ tệp nào đang mở.
• Đã cập nhật mục Xem > Phông chữ: thay bộ chọn đầy đủ bằng submenu nhanh với các phông phổ biến (Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman, Georgia), đồng thời giữ nguyên cỡ chữ hiện tại.
• Đã cải thiện cách đọc RSS và podcast với hai kiểu thông báo tách biệt: nút nguồn sẽ báo "mục mới" khi feed/podcast có nội dung mới, còn từng bài RSS và từng tập podcast sẽ báo "chưa đọc"/"chưa phát"; có thể tắt hành vi này trong Tùy chọn.
Sửa lỗi
• Đã sửa trích xuất văn bản EPUB cho sách có chứa chú thích HTML inline (<!-- ... -->): văn bản chương giờ được phân tích đúng thay vì bị bỏ qua một phần hoặc toàn bộ.
• Đã sửa từ điển Wiktionary tiếng Tây Ban Nha và cơ chế cache từ điển: các từ như "agua" giờ được tìm đúng, và các mục cache cũ kiểu "Không tìm thấy từ" sẽ không còn được tái sử dụng.
• Đã sửa mã hóa khi nhập bài RSS cho một số nguồn tiếng Tây Ban Nha (ví dụ El Mundo): dấu tiếng Tây Ban Nha và ký tự "ñ" giờ được giữ đúng trong trình soạn thảo tạm.
• Đã sửa giải mã ANSI cho các tệp Trung Âu (ví dụ tiếng Séc/tiếng Ba Lan): Sonarpad giờ phân biệt UTF-8 và ANSI tốt hơn và chọn đúng bảng mã (bao gồm Windows-1250), tránh lỗi vỡ dấu.
• Đã sửa lỗi lưu nguồn RSS có tham số trong URL (ví dụ `rss.aspx?c=...`): các feed này giờ được lưu và khôi phục đúng sau khi khởi động lại Sonarpad.
• Đã sửa lỗi mở các tệp con trỏ Google Drive (`.gdoc`, `.gsheet`, `.gslides`) từ menu ngữ cảnh Explorer: khi đọc trực tiếp bị lỗi “Incorrect function (os error 1)”, Sonarpad giờ dùng fallback shell-open để tài liệu vẫn mở đúng.
• Đã sửa đọc tệp Excel legacy `.xls` (Excel 2010): các tệp nhị phân cũ giờ được nhận diện/giải mã đúng thay vì hiển thị văn bản lỗi (ví dụ `ÐÏ_à¡±...`).
• Đã sửa luồng thông báo lỗi chính tả: lỗi sẽ được đọc lại khi bạn rà soát văn bản sau đó, và cùng một lỗi sẽ được báo lại nếu bị xóa rồi gõ lại.
• Đã sửa các thao tác văn bản theo dòng (ví dụ Ctrl+Q / Ctrl+Shift+Q, sắp xếp/đảo/duy nhất/gộp dòng): khi chọn một dòng bằng Shift+Mũi tên xuống, các dòng liền kề không còn bị dính hoặc bị cắt.
• Đã sửa xử lý chọn nhiều dòng cho các thao tác theo dòng (Ctrl+Q / Ctrl+Shift+Q và các công cụ liên quan): khi RichEdit trả về dấu xuống dòng dạng CR-only, Sonarpad giờ chuẩn hóa đúng để xử lý đủ tất cả dòng đã chọn mà không cắt mất ký tự đầu dòng.
• Đã mở rộng chuẩn hóa đầu vào TTS cho các ký hiệu hiển thị của khoảng trắng/tab/xuống dòng (␠/U+2420, ␣/U+2423, ␉/U+2409, ␊/U+240A, ␍/U+240D, ␤/U+2424), vốn có thể gây lặp đoạn với giọng đa ngôn ngữ.
• Đã tinh chỉnh bước làm sạch văn bản cho Edge TTS bằng một pipeline kiểm tra duy nhất: chuẩn hóa khoảng trắng lạ/vô hình, rút gọn các chuỗi dấu câu dài (như "...", "!!!", "???"), và bỏ qua các đoạn chỉ gồm dấu câu để tránh lặp vòng khi phát.
• Đã sửa thông báo thời gian phát (Ctrl+I) cho luồng MP3/podcast: thời gian hiện tại giờ được giới hạn theo tổng thời lượng của track, và phát sẽ tự dừng nếu vị trí vượt quá điểm kết thúc.
• Đã cải thiện phạm vi bản địa hóa của trình cài đặt: setup.exe giờ bao gồm thêm tiếng Séc, Ba Lan, Pháp và Serbia, trong khi MSI được giữ thành một gói en-US duy nhất để tránh gây rối trong bản phát hành.
• Đã sửa dọn dẹp khi gỡ cài đặt các mục menu ngữ cảnh: mục "Mở bằng Sonarpad" giờ được xóa ổn định, kể cả trong các kịch bản registry cũ.
• Đã sửa độ ổn định của tạm dừng/tiếp tục với SAPI5: tạm dừng bằng F4 giờ hoạt động đúng và khi tiếp tục sẽ quay lại đúng vị trí mong đợi thay vì phát lại từ đầu.
• Đã sửa luồng tạm dừng + tua + tiếp tục khi phát media: sau khi tạm dừng và tua bằng Trái/Phải, nhấn Space giờ sẽ tiếp tục ổn định tại vị trí hiện tại thay vì dừng hẳn hoặc phát lại từ đầu.

Phiên bản 0.6.5 – 2026-02-07
Cải tiến
• Bản dịch tiếng Tây Ban Nha được cải thiện nhờ Arturo Fernandez Rivas.
• Đã thêm tùy chọn tách sách nói EPUB theo chương.
• Nhập RSS giờ dùng tab tạm riêng (tiêu đề đã bản địa hóa); Lưu dưới dạng sẽ chuyển thành tài liệu bình thường.
• Thông báo từ trình đọc màn hình giờ cũng được gửi tới JAWS khi có sẵn.
Sửa lỗi
• Đọc từ vị trí con trỏ (F5) giờ bắt đầu chính xác tại con trỏ. Trước đây có thể bắt đầu vài dòng phía trên vì offset con trỏ không khớp với vị trí CRLF/UTF-16.
• Đã sửa lỗi vẽ lại: khi gõ đè lên vùng chọn, phần văn bản phía trước có thể biến mất cho tới khi di chuyển vùng chọn.
• Sửa parser chương EPUB: trang bìa hoặc chỉ có hình ảnh không còn bị đọc CSS (ví dụ "padding") hay tiêu đề "Sconosciuto".
• Đã sửa lỗi chia audiobook từ EPUB theo thời gian: Edge TTS có thể lỗi khi gặp đoạn rỗng hoặc quá dài ("Edge audio not sent").
• Bài viết RSS giờ giải mã các thực thể HTML (ví dụ &quot;, &amp;, &lt;, &gt;).
• Lưu/Lưu dưới tên giờ đây đề xuất tên tệp hiện có khi lưu các định dạng không nên ghi đè (ví dụ: EPUB), thay vì dòng đầu tiên.
• Đã sửa lỗi khiến podcast có tập mới không được thông báo là chưa phát, đồng thời đổi "Chưa nghe" thành "Chưa phát" cho chuyên nghiệp hơn.

Phiên bản 0.6.4 – 2026-02-05
Cải tiến
• Chương trình đã được đổi tên thành Sonarpad để nhấn mạnh âm thanh và audio, là điểm then chốt của chương trình.
• Thêm lựa chọn track âm thanh trong menu Phát lại cho các tệp đa phương tiện có nhiều track âm thanh (ví dụ: MKV với nhiều ngôn ngữ).
• Podcast giờ đây hiển thị rõ ràng những tập chưa nghe với tiền tố "Chưa nghe" trước tên.
• Hệ thống thẻ mới để đổi giọng trong văn bản. Ví dụ:
  - Giọng Microsoft (Edge): <voice edge it-IT-IsabellaNeural>Xin chào</voice>
  - Giọng SAPI5: <voice sapi5 Microsoft Helena Desktop>Xin chào</voice>
  - Giọng SAPI4: <voice sapi4 #1>Xin chào</voice>
• Bổ sung danh mục podcast.
• Thêm tùy chọn trong menu ngữ cảnh để tạo sách nói từ phần chọn.
Sửa lỗi
• Khắc phục lỗi khiến sách nói SAPI4 có thể được tạo ra khác với mong đợi.
• Cửa sổ Tìm trong tệp: nhấn Enter trên một kết quả giờ mở đúng vị trí đoạn trích và Esc quay lại kết quả.
• Cửa sổ Tùy chọn: chỉnh lại bố cục hiển thị ở các tab Chung, Giọng nói, Trình soạn thảo và Âm thanh để tránh thiếu hoặc bị cắt điều khiển.
• Đã sửa lỗi dấu trang khi thay đổi tốc độ phát.
• Đã sửa lỗi Podcast Index và danh mục không hiển thị đúng.
• Đã sửa lỗi dấu nháy đơn làm ngắt đọc: không còn chế độ đọc riêng cho hội thoại, dùng thẻ giọng nói.

Phiên bản 0.6.3 – 2026-01-30
Cải tiến
• Cải thiện phát hiện micrô.
• Thêm phát lại tức thì cho tất cả các định dạng.
Sửa lỗi
• Sửa lỗi sập trong cửa sổ danh mục podcast.

Phiên bản 0.6.2 – 2026-01-30
Tính năng mới
• Thêm hỗ trợ chạy tệp (Shift+F5). Người dùng có thể chọn trình thông dịch (ví dụ: python) trong Tùy chọn, tìm kiếm trên máy tính, và nhấn Shift+F5 để chạy tập lệnh hiện tại. Các tệp HTML mở trong trình duyệt.
• Thêm hỗ trợ cho các tệp con trỏ Google Docs (.gdoc, .gsheet, .gslides), tự động mở trong trình duyệt mặc định.
• Thêm hỗ trợ định dạng sách nói M4B (Apple/AAC).
• Thêm tùy chọn "Hiển thị tập" trong menu ngữ cảnh kết quả tìm kiếm podcast để duyệt và phát các tập mà không cần đăng ký.
• Thêm tính năng "Đi đến dòng" (menu Chỉnh sửa hoặc Ctrl+J) để nhảy nhanh đến số dòng cụ thể.
• Thêm tùy chọn menu ngữ cảnh để sắp xếp nguồn cấp RSS và podcast (theo bảng chữ cái hoặc theo ngày).
• Thêm nguồn cấp RSS mặc định bằng tiếng Việt.
• Thêm hộp kiểm tra micrô trong hộp thoại ghi âm để kiểm tra mức độ trước khi bắt đầu.
• Thêm "Hiển thị mô tả" cho các tập podcast trong menu ngữ cảnh.
• Thêm hỗ trợ cho các định dạng âm thanh/video mở rộng qua FFmpeg: mkv, avi, mov, m4v, webm, mpg, ts, wmv, flv, vob, 3gp, flac, ogg, wma, aiff.
• Thêm đọc phụ đề đồng bộ (srt, vtt, ass, sub, sbv, lrc, smi) với NVDA hoặc giọng đã chọn. Chương trình tìm kiếm tệp phụ đề có cùng tên với tệp phương tiện. Thêm tùy chọn "Nhập phụ đề" và "Xóa phụ đề" trong menu Phát lại cho các tệp có tên khác nhau.
• Thêm liên kết tệp cho tất cả các định dạng âm thanh/video mới được hỗ trợ trong menu ngữ cảnh "Mở bằng Sonarpad".
• Thêm cài đặt để điều chỉnh cao độ của bất kỳ tệp nào.
• Thêm tùy chọn trong Cài đặt Chung để bật hoặc tắt báo cáo lỗi ẩn danh. Thêm mục trong menu Trợ giúp để tạo tệp ZIP chẩn đoán.
• Thêm tùy chọn sử dụng giọng khác cho đối thoại, cả cho đọc trực tiếp và tạo sách nói.
• Thêm trình duyệt danh mục podcast để khám phá podcast theo danh mục (kinh doanh, nghệ thuật, thể thao, v.v.).
Cải tiến
• Mở tệp âm thanh/video từ Explorer giờ mở trực tiếp giao diện trình phát thay vì trình soạn thảo văn bản.
• Loại bỏ yêu cầu OCR cho PDF không thể truy cập; OCR giờ được thực hiện tự động để cải thiện tốc độ và trải nghiệm người dùng.
• Cải thiện Terminal Trợ năng: đọc NVDA giờ nhớ dòng cuối cùng đã đọc để liên tục tốt hơn.
• SAPI 4: Tạo sách nói giờ hoàn toàn song song và gần như tức thì. Thêm yêu cầu để chọn số quy trình đồng thời.
• SAPI 4: Loại bỏ nút cổ chai WAV-MP3 bằng cách chuyển đổi các đoạn song song trong quá trình tổng hợp.
• SAPI 4: Cải thiện xử lý lỗi và dọn dẹp tự động các tệp tạm thời.
• Hộp thoại Tìm: Đổi tên "Regex" thành "Biểu thức chính quy" để rõ ràng hơn và thêm các bản dịch còn thiếu cho các tùy chọn tìm kiếm.
• Sách nói M4B: Xử lý đầu ra tốt hơn; chia theo phần/đánh dấu giờ tạo ra một tệp M4B duy nhất với siêu dữ liệu chương bao gồm tiêu đề và tác giả.
• Trình phát: Sửa độ chính xác của dấu trang và thông báo thời gian khi tốc độ phát không phải 1.0x.
• Khôi phục điều hướng Ctrl+Tab và Ctrl+Shift+Tab trong Tùy chọn.
• Thêm tùy chọn trong menu Phát lại để đặt lại ngay tốc độ về Bình thường (1.0x).
• Cập nhật tất cả các phụ thuộc lên phiên bản mới nhất để có hiệu suất và độ ổn định tốt hơn.
• Tích hợp FFmpeg với tải DLL động để đảm bảo khả năng tương thích mà không chặn khởi động.
• Cập nhật bộ lọc tải xuống podcast để bao gồm các định dạng âm thanh/video mới.
• Ngăn Ctrl+S lưu các tệp âm thanh/video để tránh hỏng.
• Cải thiện nhập bản ghi YouTube làm cho nó mạnh mẽ và linh hoạt hơn.
• Cải thiện độ bền của việc chia sách nói thành các phần, đảm bảo không mất văn bản nào.
• Trình cài đặt giờ hoàn toàn đa ngôn ngữ, hỗ trợ tiếng Ý, Anh, Tây Ban Nha, Bồ Đào Nha, Thụy Điển và Việt Nam dựa trên ngôn ngữ hệ thống của người dùng. Tiếng Anh là mặc định cho các hệ thống không được hỗ trợ.
• Danh mục podcast: nhấn Enter trên một danh mục giờ xác nhận lựa chọn (tương đương nút OK).
• Cải thiện hệ thống phát hiện treo để tránh dương tính giả khi có hộp thoại modal mở (thông báo lỗi, "không tìm thấy văn bản").
Sửa lỗi
• Sửa lỗi changelog không mở khi khởi động.
• Sửa lỗi yêu cầu OCR không xuất hiện cho PDF không thể truy cập được mở từ Explorer.
• Sửa lỗi khởi động có thể gây mất tiêu điểm hoặc đóng cửa sổ ngay sau khi mở.
• Sửa lỗi nghiêm trọng trong tìm kiếm regex ngăn tìm văn bản, bao gồm các vấn đề với "Tìm kiếm vòng" và tùy chọn "Dấu chấm tương đương dòng mới" với kết thúc dòng Windows.
Bản địa hóa
• Thêm bản dịch tiếng Ba Lan.
• Thêm bản dịch tiếng Pháp.
• Thêm bản dịch tiếng Séc (cảm ơn Radek Žalud và Jiri Holzinger).

Phiên bản 0.6.1 – 2026-01-20

Sửa lỗi

• Đã sửa lỗi khi bật “Hiển thị giọng nói trong trình soạn thảo” và phát podcast thì quá trình phát bị dừng.

• Đã sửa lỗi khiến một số podcast không thể thêm bằng URL do địa chỉ bị cắt ngắn.

• Đã sửa lỗi không thể thêm các URL thông thường trong chức năng RSS feed.

• Đã sửa lỗi khiến ngôn ngữ Wikipedia hiển thị ở nhiều tab cài đặt khác nhau.

• Đã loại bỏ việc tạo một số tệp debug vốn vẫn được tạo ngay cả ở chế độ release.

Cải tiến

• Cải thiện hỗ trợ cho giọng nói Microsoft, hiện được phát bằng phương thức chuyên biệt với user agent khác.

• Đã thêm hỗ trợ cho tệp MP4.

Phiên bản 0.6.0 – 2026-01-20
Tính năng mới
• Thêm trình kiểm tra chính tả. Từ menu chuột phải, người dùng có thể kiểm tra xem từ hiện tại có đúng không và nếu không, sẽ nhận được các gợi ý sửa lỗi.
• Thêm tính năng nhập và xuất podcast thông qua tệp OPML.
• Thêm hỗ trợ tìm kiếm trên Podcast Index cùng với iTunes. Người dùng có thể nhập API key và secret miễn phí (tạo chỉ bằng địa chỉ email).
• Thêm hỗ trợ cho các giọng đọc SAPI4, áp dụng cho cả việc đọc thời gian thực và tạo sách nói.
• Thêm tính năng tự động chuyển sang OCR cho các tệp PDF không hỗ trợ tiếp cận: khi không tìm thấy văn bản có thể trích xuất, tài liệu sẽ được nhận dạng qua OCR.
• Thêm hỗ trợ từ điển bằng Wiktionary. Nhấn phím Applications sẽ hiển thị các định nghĩa, và khi có sẵn, sẽ hiện cả từ đồng nghĩa cùng bản dịch sang các ngôn ngữ khác.
• Thêm tính năng nhập bài viết Wikipedia với khả năng tìm kiếm, chọn kết quả và nhập trực tiếp vào trình soạn thảo.
• Thêm phím tắt Shift+Enter trong mô-đun RSS để mở trực tiếp bài viết trên trang web gốc.
Cải tiến
• Việc lựa chọn Micro giờ đây luôn được ứng dụng tuân thủ chính xác.
• Trong cửa sổ podcast, nhấn Enter vào một tập tin giờ đây sẽ thông báo ngay lập tức "đang tải" qua NVDA để xác nhận thao tác.
• Trong kết quả tìm kiếm podcast, nhấn Enter giờ đây sẽ đăng ký theo dõi podcast đã chọn.
• Sửa và cải thiện các nhãn cho phím tắt Ctrl+Shift+O và Podcast Ctrl+Shift+P.
• Tốc độ phát và âm lượng giờ đây được lưu trong cài đặt và duy trì cho tất cả các tệp âm thanh.
• Thêm một thư mục bộ nhớ đệm (cache) riêng cho các tập podcast. Người dùng có thể giữ lại các tập phim qua mục "Giữ podcast" trong menu Phát lại. Bộ nhớ đệm sẽ tự động được dọn dẹp khi vượt quá kích thước do người dùng thiết lập (Tùy chọn → Âm thanh).
• Cải thiện đáng kể việc tải bài viết RSS bằng cách sử dụng giả lập libcurl với cấu hình Chrome và iPhone, đảm bảo tương thích với khoảng 99% các trang web.
• Thêm trạng thái đã đọc / chưa đọc cho các bài viết RSS, với chỉ báo rõ ràng trong danh sách RSS.
• Tính năng Thay thế tất cả giờ đây sẽ báo cáo số lượng thay thế đã thực hiện.
• Thêm nút Xóa Podcast khi điều hướng thư viện podcast bằng phím Tab.
Sửa lỗi
• Loại bỏ mục "bản cập nhật đang chờ" thừa trong menu Trợ giúp (việc cập nhật đã được xử lý tự động).
• Sửa lỗi nhấn Ctrl+S trên tệp MP đang mở gây lưu đè và làm hỏng tệp.
• Sửa lỗi giao diện khiến "Sách nói hàng loạt" hiển thị thành "(B)… Ctrl+Shift+B" (loại bỏ nhãn thừa).
• Sửa lỗi ngoặc kép thông minh: khi được bật, các dấu ngoặc kép thông thường giờ đây sẽ được thay thế chính xác bằng ngoặc kép thông minh.
• Sửa lỗi sử dụng "Đi tới dấu trang" làm đặt lại tốc độ phát về 1.0.
• Sửa lỗi các tập podcast đã tải về lại bị tải lại thay vì sử dụng bản lưu trong bộ nhớ đệm.
Phím tắt
• F1 giờ đây mở hướng dẫn Trợ giúp.
• F2 giờ đây kiểm tra các bản cập nhật.
• F7 / F8 giờ đây nhảy đến lỗi chính tả trước đó hoặc tiếp theo.
• F9 / F10 giờ đây chuyển đổi nhanh giữa các giọng đọc yêu thích.
Cải tiến cho nhà phát triển
• Các lỗi không còn bị bỏ qua một cách im lặng: tất cả các mẫu `let _ =` đã bị loại bỏ, và các lỗi hiện được xử lý rõ ràng (truyền đi, ghi nhật ký hoặc xử lý bằng các phương án dự phòng phù hợp).
• Dự án giờ đây sẽ không thể biên dịch nếu có cảnh báo (warnings): cả `cargo check` và `cargo clippy` phải vượt qua một cách sạch sẽ.
• Các triển khai tùy chỉnh như strlen / wcslen đã bị loại bỏ. Độ dài chuỗi và bộ đệm UTF-16 giờ đây được lấy trực tiếp từ dữ liệu do Rust quản lý thay vì quét bộ nhớ.
• Việc xử lý DLL đã được làm sạch và hợp nhất xung quanh `libloading`, tránh các logic trình nạp tùy chỉnh và phân tích cú pháp PE.
• Các trình hỗ trợ phân tích cú pháp byte tự viết đã bị loại bỏ; tất cả việc phân tích byte hiện sử dụng `from_le_bytes` / `from_be_bytes` tiêu chuẩn trên các lát cắt (slices) đã được kiểm tra.
Những thay đổi này giúp giảm việc sử dụng mã không an toàn (unsafe) không cần thiết, loại bỏ các hành vi không xác định tiềm ẩn và làm cho mã nguồn trở nên chuẩn mực, mạnh mẽ và dễ bảo trì hơn.

Phiên bản 0.5.9 - 2026-01-13
Tính năng mới
• Thêm tính năng sắp xếp lại RSS từ menu chuột phải (lên/xuống/đến vị trí) với kiểm tra vị trí hợp lệ.
• Thêm menu ngữ cảnh cho bài viết với các tùy chọn mở trang web gốc và chia sẻ qua WhatsApp, Facebook và X.
• Thêm phím tắt Esc để quay lại danh sách RSS từ các bài viết đã nhập.
• Thêm chế độ podcast: tìm kiếm, đăng ký, lắng nghe; sắp xếp lại các đăng ký; phím Esc dừng phát và quay lại danh sách; phím Enter trên một tập phim để bắt đầu phát.
• Thêm điều khiển tốc độ phát cho podcast và tệp MP3.
• Thêm Ctrl+T để đi tới một mốc thời gian cụ thể.
• Thêm nút nghe thử giọng đọc sau hộp chọn âm lượng.
• Thêm tính năng tìm kiếm và thay thế bằng biểu thức chính quy (Regex) theo phong cách Notepad++.
• Thêm tính năng nhập RSS từ tệp OPML và TXT.
• Thêm tùy chọn để bật "Mở bằng Sonarpad" trong File Explorer, bao gồm cả các bản portable.
Cải tiến
• Cải thiện việc chọn tốc độ/cao độ/âm lượng giọng đọc, tuân thủ các giới hạn tối đa của TTS.
• Nhiều cải tiến RSS để tải xuống tất cả bài viết mà không làm di chuyển tiêu điểm NVDA trong quá trình cập nhật.
• Cải thiện việc phát âm thanh với menu chuyên dụng, thông báo thời gian bằng Ctrl+I và âm lượng lên tới 300%.
• Thêm các phím tắt còn thiếu cho một số chức năng.
• Tổ chức lại menu Chỉnh sửa với menu con dọn dẹp văn bản.
• Tổ chức lại Tùy chọn thành các tab, với điều hướng bằng Ctrl+Tab và Ctrl+Shift+Tab.
• Trình đọc RSS hiện tải toàn bộ nội dung bài viết, khớp với chế độ xem trên trình duyệt.
Sửa lỗi
• Sửa lỗi dọn dẹp Markdown làm xóa mất các số ở đầu dòng.
• Sửa lỗi AltGr+Z kích hoạt lệnh hoàn tác.
• Sửa lỗi hủy ghi âm sách nói để quá trình dừng lại nhanh chóng.
Bản địa hóa
• Thêm bản dịch tiếng Việt (cảm ơn Anh Đức Nguyễn).

Phiên bản 0.5.8 - 2026-01-10
Tính năng mới
• Thêm điều khiển âm lượng cho micro và âm thanh hệ thống khi ghi âm podcast.
• Thêm tính năng mới để nhập bài viết từ các trang web hoặc nguồn cấp dữ liệu RSS, bao gồm các nguồn quan trọng nhất cho mỗi ngôn ngữ.
• Thêm chức năng xóa tất cả dấu trang cho tệp hiện tại.
• Thêm chức năng xóa các dòng trùng lặp và các dòng trùng lặp liên tiếp.
• Thêm chức năng đóng tất cả các tab hoặc cửa sổ ngoại trừ cái hiện tại.
• Thêm mục Quyên góp trong menu Trợ giúp cho tất cả các ngôn ngữ.
Cải tiến
• Cải thiện terminal hỗ trợ tiếp cận để ngăn chặn một số lỗi treo máy.
• Cải thiện và sửa lỗi các phím truy cập và phím tắt trong toàn bộ ứng dụng.
• Sửa lỗi đóng cửa sổ phát âm thanh nhưng âm thanh không dừng.
• Thêm hộp thoại xác nhận cho các hành động quan trọng (ví dụ: xóa dòng trùng lặp, xóa dấu gạch nối cuối dòng, xóa tất cả dấu trang). Không có hộp thoại nào hiển thị khi hành động đó không thể thực hiện.
• Thêm khả năng xóa các nguồn RSS/trang web khỏi thư viện bằng cách chọn chúng và nhấn phím Delete.
• Thêm menu chuột phải trong cửa sổ RSS để chỉnh sửa hoặc xóa các nguồn RSS/trang web.
• Loại bỏ cài đặt di chuyển cài đặt sang thư mục hiện tại; ứng dụng hiện tự động xử lý việc này dựa trên vị trí (nếu thư mục chứa file exe tên là "sonarpad portable" hoặc nằm trên ổ đĩa di động, cài đặt sẽ vào thư mục `config` của exe, nếu không sẽ vào `%APPDATA%\Sonarpad`).

Phiên bản 0.5.7 - 2026-01-05
Tính năng mới
• Thêm tính năng Sách nói hàng loạt để chuyển đổi nhiều tệp/thư mục cùng lúc.
• Thêm hỗ trợ cho các tệp Markdown (.md).
• Thêm lựa chọn bảng mã khi mở các tệp văn bản.
• Thêm tùy chọn trong terminal hỗ trợ tiếp cận để thông báo khi có dòng mới bằng NVDA.
Cải tiến
• Ghi âm sách nói giờ đây lưu trực tiếp sang MP3 khi được chọn.
• Người dùng giờ đây có thể chọn vị trí dấu sao (*) báo hiệu chưa lưu trên tiêu đề cửa sổ.
• Cải thiện độ ổn định của hệ thống cập nhật.
• Thêm mục "Xóa dấu gạch nối" trong menu Chỉnh sửa để sửa lỗi ngắt dòng OCR.

Phiên bản 0.5.6 - 2026-01-04
Sửa lỗi
  Cải thiện Tìm trong các tệp để nhấn Enter sẽ mở tệp chính xác tại đoạn văn bản đã chọn.
Cải tiến
  Thêm hỗ trợ PPT/PPTX (mở dưới dạng văn bản).
  Mở các định dạng không phải văn bản giờ đây sẽ lưu thành .txt để tránh lỗi định dạng (PDF/DOC/DOCX/EPUB/HTML/PPT/PPTX).
  Thêm ghi âm podcast từ micro và âm thanh hệ thống (Menu Tệp, Ctrl+Shift+R).

Phiên bản 0.5.5 – 2026-01-03
Tính năng mới
• Thêm terminal hỗ trợ tiếp cận được tối ưu hóa cho đầu ra lớn và trình đọc màn hình (Ctrl+Shift+P).
• Thêm cài đặt để lưu cài đặt người dùng trong thư mục hiện tại (chế độ portable).
Sửa lỗi
• Cải thiện đoạn trích dẫn Tìm trong các tệp để phần xem trước luôn khớp với kết quả tìm thấy.

Phiên bản 0.5.4 – 2026-01-03
Cải tiến
• Sửa lỗi Chuẩn hóa khoảng trắng (Ctrl+Shift+Enter).
• Thêm hỗ trợ HTML/HTM (mở dưới dạng văn bản).

Phiên bản 0.5.3 – 2026-01-02
Tính năng mới
• Thêm tính năng Tìm trong các tệp.
• Thêm các công cụ văn bản mới: Chuẩn hóa khoảng trắng, Ngắt dòng cứng và Loại bỏ Markdown.
• Thêm Thống kê văn bản (Alt+Y).
• Thêm các lệnh danh sách mới trong menu Chỉnh sửa:
• Sắp xếp các mục (Alt+Shift+O)
• Giữ lại các mục duy nhất (Alt+Shift+K)
• Đảo ngược các mục (Alt+Shift+Z)
• Thêm Trích dẫn / Bỏ trích dẫn các dòng (Ctrl+Q / Ctrl+Shift+Q).
Bản địa hóa
• Thêm bản dịch tiếng Tây Ban Nha.
• Thêm bản dịch tiếng Bồ Đào Nha.
Cải tiến
• Khi mở tệp EPUB, lệnh Lưu giờ đây tự động chuyển thành Lưu mới thành và xuất nội dung dưới dạng tệp .txt để tránh làm hỏng EPUB.

## 0.5.2 - 2026-01-01
- Thêm nhật ký thay đổi.
- Thêm các tùy chọn mở bằng Sonarpad và liên kết tệp trong khi cài đặt.
- Cải thiện bản địa hóa thông báo (lỗi, hộp thoại, xuất sách nói).
- Thêm lựa chọn phần khi dùng "Chia nhỏ sách nói dựa trên văn bản", với tùy chọn "Bắt buộc dấu đánh dấu ở đầu dòng".
- Thêm tính năng nhập phụ đề YouTube với lựa chọn ngôn ngữ, mốc thời gian và cải thiện xử lý tiêu điểm.

## 0.5.1 - 2025-12-31
- Cập nhật tự động có xác nhận, cải thiện thông báo và xử lý lỗi.
- Cải tiến xuất sách nói (chia nhỏ theo văn bản, SAPI5/Media Foundation, điều khiển nâng cao).
- Cải tiến TTS (tạm dừng/tiếp tục, từ điển thay thế, danh sách yêu thích).
- Menu Hiển thị và các bảng giọng đọc/yêu thích, màu chữ và cỡ chữ.
- Ngôn ngữ mặc định theo hệ thống và cải thiện bản địa hóa.
- Đóng gói cho Windows (MSI/NSIS).

## 0.5.0 - 2025-12-27
- Tái cấu trúc theo mô-đun (trình soạn thảo, xử lý tệp, menu, tìm kiếm).
- Quy trình đóng gói trên Windows và cập nhật README/giấy phép.
- Sửa lỗi điều hướng phím TAB trong cửa sổ Trợ giúp.

## 0.5 - 2025-12-27
- Nâng cấp phiên bản sơ bộ.

## 0.1.0 - 2025-12-25
- Phiên bản phát hành đầu tiên: Cấu trúc dự án và tệp README.
