/**
 * Заголовки авторизации к API Tensa (Контракт 12: `security: BearerAuth`).
 *
 * Значение токена приходит из настроек пользователя (localStorage) в момент
 * запроса: PWA не хранит токен в коде и не подставляет его из сборки.
 */

/** Имя заголовка авторизации. */
export const AUTH_HEADER = 'Authorization';

/** Схема авторизации по контракту. */
export const AUTH_SCHEME = 'Bearer';

/** Значение заголовка для токена доступа; пустой токен → `null` (заголовок не нужен). */
export function authValue(access: string | null | undefined): string | null {
  const value = access ? String(access).trim() : '';
  if (!value) {
    return null;
  }
  return `${AUTH_SCHEME} ${value}`;
}

/**
 * Дополнить набор заголовков авторизацией, если токен задан.
 *
 * Возвращает новый объект: исходный словарь не мутируется.
 */
export function withAuth(
  base: Record<string, string>,
  access: string | null | undefined,
): Record<string, string> {
  const value = authValue(access);
  if (!value) {
    return { ...base };
  }
  const headers: Record<string, string> = { ...base };
  headers[AUTH_HEADER] = value;
  return headers;
}
