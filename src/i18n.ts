import { useEffect } from 'react';
import { useStore } from './store/useStore';

export type Language = 'en' | 'uk';

const uk: Record<string, string> = {
  'Dashboard': 'Панель керування', 'Platforms': 'Платформи', 'Overlay': 'Оверлей', 'Settings': 'Налаштування', 'About': 'Про застосунок',
  'Main navigation': 'Головна навігація', 'InstantDelay Home': 'На головну InstantDelay', 'Close sidebar': 'Закрити бічну панель',
  'Buffering...': 'Буферизація...', 'Live': 'У прямому ефірі', 'Offline': 'Не в мережі', 'Click to start': 'Натисніть, щоб запустити',
  'Quick Actions': 'Швидкі дії', 'Toggle Delay': 'Увімкнути/вимкнути затримку', 'Start server first': 'Спочатку запустіть сервер',
  'Skip to Live': 'Перейти до прямого ефіру', 'Not buffering': 'Буферизація не виконується', 'Collapse sidebar': 'Згорнути бічну панель', 'Toggle sidebar': 'Перемкнути бічну панель',
  'Monitor and control your stream delay': 'Перегляд і керування затримкою трансляції', 'Manage streaming destinations': 'Керування платформами трансляції',
  'Configure LIVE/DELAYED overlay': 'Налаштування оверлею LIVE/DELAYED', 'Configure application settings': 'Налаштування застосунку',
  'Learn more about InstantDelay': 'Дізнайтеся більше про InstantDelay', 'Server Running': 'Сервер працює', 'Server Stopped': 'Сервер зупинено',
  'Notifications': 'Сповіщення', 'Clear all': 'Очистити всі', 'Close notifications': 'Закрити сповіщення', 'No notifications': 'Немає сповіщень', 'Dismiss': 'Закрити',
  'Switch to light mode': 'Увімкнути світлу тему', 'Switch to dark mode': 'Увімкнути темну тему', 'User menu': 'Меню користувача',
  'Streamer': 'Стрімер', 'GitHub Repository': 'Репозиторій GitHub', 'Quit Application': 'Закрити застосунок',
  'Monitor and control your stream delay in real-time': 'Переглядайте й керуйте затримкою трансляції в реальному часі',
  'Start Server': 'Запустити сервер', 'Stop Server': 'Зупинити сервер', 'Current Delay': 'Поточна затримка', 'seconds': 'секунд',
  'Buffer Fill': 'Заповнення буфера', 'Ready for delayed output': 'Готово до трансляції із затримкою', 'Delay Mode': 'Режим затримки',
  'Instant': 'Миттєво', 'Normal': 'Звичайний', 'Disabled': 'Вимкнено', 'Delay Controls': 'Керування затримкою',
  'Increase Delay': 'Збільшити затримку', 'Decrease Delay': 'Зменшити затримку', 'Server Status': 'Стан сервера', 'System Information': 'Відомості про систему',
  'Platform Status': 'Стан платформ', 'No platforms configured': 'Платформи не налаштовані', 'Add a platform to get started': 'Додайте платформу, щоб почати',
  'Add Platform': 'Додати платформу', 'Edit Platform': 'Редагувати платформу', 'Remove': 'Видалити', 'Edit': 'Редагувати', 'Test Connection': 'Перевірити з’єднання',
  'Enabled': 'Увімкнено', 'Name': 'Назва', 'Platform': 'Платформа', 'Stream URL': 'URL трансляції', 'Stream Key': 'Ключ трансляції',
  'Cancel': 'Скасувати', 'Save': 'Зберегти', 'Saving...': 'Збереження...', 'Save All': 'Зберегти все', 'General': 'Загальні', 'RTMP Server': 'RTMP-сервер',
  'Hotkeys': 'Гарячі клавіші', 'Advanced': 'Додатково', 'Application': 'Застосунок', 'Theme': 'Тема', 'dark': 'Темна', 'light': 'Світла', 'system': 'Системна',
  'Language': 'Мова', 'English': 'Англійська', 'Ukrainian': 'Українська', 'Auto-start server': 'Автозапуск сервера',
  'Start RTMP server on application launch': 'Запускати RTMP-сервер разом із застосунком', 'Minimize to tray': 'Згортати в системний трей',
  'Hide window to system tray on close': 'Згортати вікно в системний трей під час закриття', 'Show notifications': 'Показувати сповіщення',
  'Display desktop notifications': 'Показувати сповіщення на робочому столі', 'Check for updates': 'Перевіряти наявність оновлень',
  'Automatically check for new versions': 'Автоматично перевіряти наявність нових версій', 'Settings saved': 'Налаштування збережено',
  'Your changes have been applied': 'Зміни застосовано', 'Failed to save settings': 'Не вдалося зберегти налаштування', 'Unknown error': 'Невідома помилка',
  'Failed to initialize': 'Не вдалося ініціалізувати застосунок', 'Check console for details': 'Перевірте консоль для подробиць',
  'Server Stats': 'Статистика сервера', 'Connections': 'З’єднання', 'Uptime': 'Час роботи', 'Data In': 'Вхідні дані', 'Data Out': 'Вихідні дані',
  'System': 'Система', 'CPU': 'Процесор', 'Memory': 'Пам’ять', 'Checking...': 'Перевірка...', 'Unknown': 'Невідомо',
  'Delay Control': 'Керування затримкою', 'Instant Mode': 'Миттєвий режим', 'Delay (seconds)': 'Затримка (секунди)',
  'Increase (+5s)': 'Збільшити (+5 с)', 'Decrease (-5s)': 'Зменшити (-5 с)', 'Platform Outputs': 'Виходи на платформи',
  'Manage Platforms': 'Керування платформами', 'Status': 'Стан', 'Data Sent': 'Надіслані дані', 'Actions': 'Дії',
  'No platform outputs configured': 'Виходи на платформи не налаштовані', 'Connection Details': 'Деталі з’єднання', 'Connected': 'Підключено', 'Disconnected': 'Відключено', 'Manage your streaming destinations': 'Керування платформами трансляції', 'Add Your First Platform': 'Додайте першу платформу',
  'Add your streaming destinations to start relaying your delayed stream to multiple platforms simultaneously.': 'Додайте платформи трансляції, щоб одночасно передавати на них потік із затримкою.',
  'Configure InstantDelay to match your workflow': 'Налаштуйте InstantDelay відповідно до вашого робочого процесу', 'Open Config Folder': 'Відкрити теку налаштувань', 'Settings tabs': 'Вкладки налаштувань',
  'OBS Input': 'Вхід OBS', 'OBS is not running': 'OBS не запущено', 'OBS is running': 'OBS запущено', 'Copy': 'Копіювати', 'Copied': 'Скопійовано',
  'minutes': 'хвилин', 'hours': 'годин',
};

export function translate(language: Language, text: string): string {
  return language === 'uk' ? (uk[text] ?? text) : text;
}

export function useTranslation() {
  const language = useStore((state) => state.language);
  const setLanguage = useStore((state) => state.setLanguage);
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);
  return {
    language,
    setLanguage,
    t: (text: string) => translate(language, text),
  };
}
