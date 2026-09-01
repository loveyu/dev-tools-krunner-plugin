import { WEB_CODECS, normalizeValue } from './codecs';
import { definitionOf } from './formats';
import type { DataValue, FormatId, NativeExecutor, NativeFormatId, WebCodec } from './types';

export async function convertText(
  text: string,
  source: FormatId,
  target: FormatId,
  nativeExecutor: NativeExecutor,
): Promise<string> {
  const value = await parseSource(text, source, nativeExecutor);
  return stringifyTarget(value, target, nativeExecutor);
}

async function parseSource(
  text: string,
  format: FormatId,
  nativeExecutor: NativeExecutor,
): Promise<DataValue> {
  const definition = definitionOf(format);
  if (!definition.canParse) {
    throw new Error('convert.errors.sourceFormatNotParsable');
  }
  if (definition.runtime === 'native') {
    const canonical = await nativeExecutor({
      direction: 'parse',
      format: format as NativeFormatId,
      payload: text,
    });
    return normalizeValue(JSON.parse(canonical) as unknown);
  }
  const codec = requireWebCodec(format);
  if (codec.parse === undefined) {
    throw new Error('convert.errors.missingSourceCodec');
  }
  return codec.parse(text);
}

async function stringifyTarget(
  value: DataValue,
  format: FormatId,
  nativeExecutor: NativeExecutor,
): Promise<string> {
  const definition = definitionOf(format);
  if (!definition.canStringify) {
    throw new Error('convert.errors.targetFormatNotStringifiable');
  }
  if (definition.runtime === 'native') {
    return nativeExecutor({
      direction: 'stringify',
      format: format as NativeFormatId,
      payload: JSON.stringify(value),
    });
  }
  const codec = requireWebCodec(format);
  if (codec.stringify === undefined) {
    throw new Error('convert.errors.missingTargetCodec');
  }
  return codec.stringify(value);
}

function requireWebCodec(format: FormatId): WebCodec {
  const codec = WEB_CODECS[format];
  if (codec === undefined) {
    throw new Error('convert.errors.webCodecMissing');
  }
  return codec;
}
