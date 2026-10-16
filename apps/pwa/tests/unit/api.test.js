/**
 * Unit-тесты API-клиента PWA (ТЗ apps/pwa: «Unit» — минимальный набор).
 * DOM не нужен: клиент работает на голом fetch, подмена — через fetchImpl.
 */
import { describe, expect, it } from 'vitest';

import { authValue, withAuth } from '../../src/lib/api/auth';
import { ApiError, createTensaClient, normalizeBaseUrl } from '../../src/lib/api/tensa';

/** Фиктивный fetch: запоминает последний запрос и отдаёт заданный ответ. */
function fakeFetch(handler) {
  const calls = [];
  const impl = async (url, init) => {
    calls.push({ url, init });
    return handler(url, init);
  };
  return { impl, calls };
}

/** JSON-ответ с заданным статусом. */
function jsonResponse(value, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

describe('auth', () => {
  it('собирает заголовок Bearer и не мутирует исходный словарь', () => {
    expect(authValue('abc')).toBe('Bearer abc');
    expect(authValue('')).toBeNull();

    const base = { Accept: 'application/json' };
    const headers = withAuth(base, 'abc');
    expect(headers.Authorization).toBe('Bearer abc');
    expect(base).toEqual({ Accept: 'application/json' });
  });
});

describe('normalizeBaseUrl', () => {
  it('пустое значение даёт базу по умолчанию', () => {
    expect(normalizeBaseUrl('')).toBe('/api/v1');
    expect(normalizeBaseUrl(null)).toBe('/api/v1');
  });

  it('убирает хвостовые слэши', () => {
    expect(normalizeBaseUrl('http://127.0.0.1:8765/api/v1/')).toBe('http://127.0.0.1:8765/api/v1');
  });
});

describe('клиент API', () => {
  it('шлёт токен и читает health', async () => {
    const { impl, calls } = fakeFetch(() => jsonResponse({ healthy: true, version: '0.1.0' }));
    const client = createTensaClient({ token: 'abc', fetchImpl: impl });

    const status = await client.health();
    expect(status.healthy).toBe(true);
    expect(calls[0].url).toBe('/api/v1/health');
    expect(calls[0].init.headers.Authorization).toBe('Bearer abc');
  });

  it('превращает {error} в ApiError со статусом', async () => {
    const { impl } = fakeFetch(() => jsonResponse({ error: 'unauthorized: неверный токен' }, 401));
    const client = createTensaClient({ token: 'bad', fetchImpl: impl });

    await expect(client.version()).rejects.toMatchObject({
      name: 'ApiError',
      status: 401,
    });
    try {
      await client.version();
      throw new Error('ожидался отказ');
    } catch (error) {
      expect(error).toBeInstanceOf(ApiError);
      expect(error.isUnauthorized).toBe(true);
      expect(String(error.message)).toContain('unauthorized');
    }
  });

  it('сетевая ошибка → ApiError со статусом 0', async () => {
    const { impl } = fakeFetch(() => {
      throw new Error('network down');
    });
    const client = createTensaClient({ fetchImpl: impl });

    try {
      await client.health();
      throw new Error('ожидался отказ');
    } catch (error) {
      expect(error).toBeInstanceOf(ApiError);
      expect(error.isNetworkError).toBe(true);
    }
  });

  it('создание заметки шлёт POST с JSON', async () => {
    const note = { id: 'n1', title: 'T', body: 'B', tags: [] };
    const { impl, calls } = fakeFetch(() => jsonResponse(note, 201));
    const client = createTensaClient({ token: 'abc', fetchImpl: impl });

    await client.createNote({ title: 'T', body: 'B', tags: [] });
    expect(calls[0].init.method).toBe('POST');
    expect(calls[0].url).toBe('/api/v1/notes');
    expect(calls[0].init.headers['Content-Type']).toBe('application/json');
    expect(JSON.parse(calls[0].init.body)).toEqual({ title: 'T', body: 'B', tags: [] });
  });
});
