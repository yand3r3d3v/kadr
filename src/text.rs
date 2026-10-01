//! The interface in a second language.
//!
//! The English text in the code is the key; a string without an entry here
//! is shown as it is. Names typed in the shell (operations, options) and the
//! ffmpeg command are never translated.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    #[default]
    En,
    Ru,
}

pub fn tr(lang: Lang, text: &str) -> &str {
    match lang {
        Lang::En => text,
        Lang::Ru => RU.iter().find(|(en, _)| *en == text).map_or(text, |(_, ru)| ru),
    }
}

#[cfg(test)]
pub fn english_keys() -> impl Iterator<Item = &'static str> {
    RU.iter().map(|(en, _)| *en)
}

const RU: &[(&str, &str)] = &[
    // Operations, as tabs.
    ("compress", "сжать"),
    ("cut", "вырезать"),
    ("audio", "звук"),
    ("convert", "контейнер"),
    ("speed", "скорость"),
    ("frame", "кадр"),
    // Field labels.
    ("file", "файл"),
    ("preset", "пресет"),
    ("resolution", "разрешение"),
    ("sound", "звук"),
    ("cursor", "курсор"),
    ("start", "начало"),
    ("end", "конец"),
    ("length", "длина"),
    ("mode", "режим"),
    ("width", "ширина"),
    ("format", "формат"),
    ("quality", "качество"),
    ("container", "контейнер"),
    ("time", "время"),
    ("output", "результат"),
    // Values.
    ("source", "исходное"),
    ("keep", "оставить"),
    ("remove", "убрать"),
    ("fast", "быстро"),
    ("exact", "точно"),
    ("as is", "как есть"),
    // Hints.
    ("enter picks another file", "enter выбирает другой файл"),
    ("type a new name", "введи новое имя"),
    ("18 (better) to 28 (smaller file)", "от 18 (лучше) до 28 (меньше файл)"),
    ("space opens the list", "пробел открывает список"),
    ("the frame height; the width follows", "высота кадра; ширина подберётся сама"),
    ("re-encoded to aac, 128k", "перекодируется в aac, 128k"),
    ("the result is silent", "результат будет без звука"),
    ("fastest encode, biggest file", "кодирует быстрее всех, файл самый большой"),
    ("much faster, bigger file", "заметно быстрее, файл больше"),
    ("the ffmpeg default", "как у ffmpeg по умолчанию"),
    ("slower, smaller file at the same crf", "дольше, файл меньше при том же crf"),
    ("slowest encode, smallest file", "кодирует дольше всех, файл самый маленький"),
    ("look around, then i sets the start, o the end", "осмотрись, потом i ставит начало, o конец"),
    ("←→ 1 s, ⇧←→ 10 s, , . one frame", "←→ 1 с, ⇧←→ 10 с, , . один кадр"),
    ("copies the stream; starts on a keyframe", "копирует поток; начинает с ключевого кадра"),
    ("re-encodes, cuts clean", "перекодирует, режет ровно"),
    ("seconds; a long gif gets heavy fast", "секунды; длинный gif быстро тяжелеет"),
    ("frames per second; 10 to 15 is the usual", "кадров в секунду; обычно от 10 до 15"),
    ("pixels; the height follows", "пиксели; высота подберётся сама"),
    ("plays everywhere", "играет везде"),
    ("the track is copied without loss", "дорожка копируется без потерь"),
    ("not needed: the audio is not re-encoded", "не нужно: звук не перекодируется"),
    ("0 (better) to 9 (smaller file)", "от 0 (лучше) до 9 (меньше файл)"),
    (
        "the streams are copied as they are, nothing is re-encoded",
        "потоки копируются как есть, без перекодирования",
    ),
    ("slower and longer", "медленнее и длиннее"),
    ("faster and shorter", "быстрее и короче"),
    ("lossless, bigger", "без потерь, файл больше"),
    ("smaller, slightly lossy", "меньше, с небольшими потерями"),
    ("enter takes it, esc drops it", "enter берёт, esc отменяет"),
    // Frames and small words.
    ("command", "команда"),
    ("error", "ошибка"),
    ("of", "из"),
    ("in", "в"),
    ("s", "с"),
    ("size", "размер"),
    ("reading the frame…", "читаю кадр…"),
    // The note about keyframes.
    ("keeps", "оставит"),
    ("from the keyframe before it, hidden from players", "от ключевого кадра до начала; плееры их не покажут"),
    ("starts at", "начнётся с"),
    (", the keyframe before it; exact mode cuts clean", ", с ключевого кадра; режим «точно» режет ровно"),
    // Problems.
    ("The end must come after the start.", "Конец должен быть позже начала."),
    ("The end is past the end of the file.", "Конец дальше, чем кончается файл."),
    ("The piece runs past the end of the file.", "Отрезок выходит за конец файла."),
    ("The output is the input file itself. Change the name.", "Результат совпадает с исходным файлом. Поменяй имя."),
    // Messages.
    ("Cancelled. The unfinished {} was removed.", "Отменено. Недописанный {} удалён."),
    ("Cancelled.", "Отменено."),
    ("The command is in the clipboard.", "Команда в буфере обмена."),
    (
        "No clipboard tool here: pbcopy, wl-copy or xclip is needed.",
        "Нет доступа к буферу обмена: нужен pbcopy, wl-copy или xclip.",
    ),
    ("{} is not a number.", "{} это не число."),
    ("{} is not a time; write 38, 0:38 or 00:00:38.", "{} это не время; пиши 38, 0:38 или 00:00:38."),
    // File exists.
    (" already exists.", " уже есть."),
    ("Press ", "Нажми "),
    (" to overwrite it, or change the name.", ", чтобы перезаписать, или поменяй имя."),
    // Running.
    ("Compressing", "Сжимаю"),
    ("Cutting", "Вырезаю из"),
    ("Making a gif from", "Делаю gif из"),
    ("Taking the audio from", "Достаю звук из"),
    ("Repacking", "Перекладываю"),
    ("Changing the speed of", "Меняю скорость"),
    ("Taking a frame from", "Беру кадр из"),
    // Done.
    ("Done:", "Готово:"),
    ("selected", "выбрано"),
    ("got", "получилось"),
    (
        "The start moved back {} s: fast mode cuts on keyframes.",
        "Начало сдвинулось на {} с: в быстром режиме резка идёт по ключевым кадрам.",
    ),
    (
        "For a clean edge pick exact mode: it re-encodes the piece.",
        "Для ровной границы выбери режим «точно»: он перекодирует отрезок.",
    ),
    ("before", "было"),
    ("after", "стало"),
    ("{}% smaller", "на {}% меньше"),
    ("{}% bigger", "на {}% больше"),
    // ffmpeg stopped.
    ("ffmpeg stopped at {} with ", "ffmpeg остановился на {} с "),
    ("a signal", "сигналом"),
    ("code", "кодом"),
    (". Its last lines:", ". Его последние строки:"),
    ("The unfinished ", "Недописанный "),
    (" was removed.", " удалён."),
    // The picker.
    ("Nothing here matches.", "Ничего не подходит."),
    ("Pick a file and the command starts to build.", "Выбери файл, и команда начнёт собираться."),
    // Too small.
    ("The window is too small: it needs", "Окну тесно: нужно"),
    ("it is", "сейчас"),
    ("now.", ""),
    ("Make it bigger and the form comes back.", "Растяни окно, форма вернётся сама."),
    // The key bar.
    ("run", "пуск"),
    ("field", "поле"),
    ("value", "значение"),
    ("operation", "операция"),
    ("copy", "копия"),
    ("keys", "клавиши"),
    ("quit", "выход"),
    ("mark", "метки"),
    ("take it", "взять"),
    ("keep the old one", "оставить прежнее"),
    ("overwrite", "перезаписать"),
    ("back to the form", "назад к форме"),
    ("cancel", "отменить"),
    ("ffmpeg output", "вывод ffmpeg"),
    ("again", "ещё раз"),
    ("another file", "другой файл"),
    ("cut exactly", "вырезать точно"),
    ("full ffmpeg output", "весь вывод ffmpeg"),
    ("copy the command", "скопировать команду"),
    ("pick", "выбрать"),
    ("filter", "фильтр"),
    ("back", "назад"),
    ("close", "закрыть"),
    ("language", "язык"),
    ("language:", "язык:"),
    // The keys page.
    ("everywhere", "везде"),
    ("in a field", "в поле"),
    ("in a time field", "в поле времени"),
    ("while ffmpeg runs", "пока идёт ffmpeg"),
    ("next and previous operation", "следующая и прошлая операция"),
    ("run the command", "запустить команду"),
    ("back; from the form, quit", "назад; из формы выход"),
    ("one step", "на шаг"),
    ("a big step", "на большой шаг"),
    ("type a value, enter takes it", "ввести значение, enter берёт"),
    ("next value, or open the list", "следующее значение или список"),
    ("back to the default", "вернуть значение по умолчанию"),
    ("±1 s", "±1 с"),
    ("±10 s", "±10 с"),
    ("one frame back and forward", "на кадр назад и вперёд"),
    ("in cut: the cursor becomes start, end", "в «вырезать»: курсор в начало, конец"),
    ("cancel, remove the unfinished file", "отменить, недописанный файл удалится"),
    ("show the ffmpeg output", "показать вывод ffmpeg"),
    ("Letter keys also work in a Cyrillic layout.", "Буквенные клавиши работают и в русской раскладке."),
    (
        "kadr adds -hide_banner -nostats -progress pipe:1 to what it runs,",
        "К запуску kadr добавляет -hide_banner -nostats -progress pipe:1",
    ),
    ("and -n unless you chose to overwrite.", "и -n, пока ты не согласился на перезапись."),
    ("ffmpeg has not said anything yet.", "ffmpeg пока ничего не сказал."),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_the_text_itself_and_unknown_text_passes_through() {
        assert_eq!(tr(Lang::En, "compress"), "compress");
        assert_eq!(tr(Lang::Ru, "compress"), "сжать");
        assert_eq!(tr(Lang::Ru, "libx264"), "libx264");
    }

    #[test]
    fn no_key_is_listed_twice() {
        for (i, (key, _)) in RU.iter().enumerate() {
            assert!(RU[..i].iter().all(|(k, _)| k != key), "{key} is in the table twice");
        }
    }

    #[test]
    fn templates_keep_their_placeholder() {
        for (en, ru) in RU {
            assert_eq!(en.matches("{}").count(), ru.matches("{}").count(), "{en}");
        }
    }
}
