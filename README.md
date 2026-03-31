# 42 KFS-1

## Tanım
**KFS-1**, 42 ekolünde gerçekleştirilen bir proje olup, temel bir dosya sistemi uygulaması geliştirmeyi amaçlar. Bu proje kapsamında, bir dosya sistemi nasıl tasarlanır, nasıl yönetilir ve nasıl optimize edilir konularında deneyim kazanılır.

## Özellikler
- Basit bir dosya sistemi mimarisi
- Dosya ve dizin oluşturma, silme ve listeleme işlemleri
- Temel veri yönetimi mekanizmaları
- Blok tahsisi ve yönetimi
- Metadata yapıları ve işleyişi

## Gereksinimler
Bu projeyi derlemek ve çalıştırmak için aşağıdaki araçlara ihtiyacınız olacak:
- **GCC veya Clang** (C dili desteği gereklidir)
- **Make**
- **Linux veya macOS** işletim sistemi (Unix tabanlı sistemler önerilir)
- **QEMU veya Bochs** (Dosya sistemini test etmek için sanal ortam)

## Kurulum
Projeyi klonlayarak başlayabilirsiniz:
```sh
$ git clone https://github.com/kullanici/42-kfs1.git
$ cd 42-kfs1
```

Daha sonra derleme işlemini başlatın:
```sh
$ make
```

## Kullanım
Derleme işlemi tamamlandıktan sonra dosya sistemini çalıştırmak için aşağıdaki komutu kullanabilirsiniz:
```sh
$ ./kfs1
```

Eğer QEMU üzerinde test etmek isterseniz:
```sh
$ make run
```

## Dosya Yapısı
Aşağıda proje dosyalarının yapısını görebilirsiniz:
```
42-kfs1/
├── src/            # Kaynak kodları içerir
├── include/        # Başlık dosyaları
├── docs/           # Dokümantasyon
├── Makefile        # Derleme betiği
└── README.md       # Bu dosya
```

## Katkıda Bulunma
Eğer projeye katkıda bulunmak isterseniz, bir **fork** oluşturabilir ve **pull request** gönderebilirsiniz. Hata raporları ve önerileriniz için **Issues** bölümünü kullanabilirsiniz.

## Lisans
Bu proje **MIT Lisansı** altında sunulmaktadır. Daha fazla bilgi için `LICENSE` dosyasına göz atabilirsiniz.

---

Herhangi bir ekleme veya değişiklik yapmak istersen bana bildirebilirsin!
