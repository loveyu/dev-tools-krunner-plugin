export type DataValue =
  null | boolean | number | string | readonly DataValue[] | { readonly [key: string]: DataValue };

export type FormatId =
  | 'json'
  | 'json-deep'
  | 'json-min'
  | 'js-object'
  | 'yaml'
  | 'xml'
  | 'csv'
  | 'tsv'
  | 'toml'
  | 'ini'
  | 'query-rfc1738'
  | 'query-rfc3986'
  | 'cookie'
  | 'postman-bulk'
  | 'line'
  | 'plain'
  | 'uri'
  | 'jwt'
  | 'base64'
  | 'base64-gzip'
  | 'url-encode'
  | 'php-serialize'
  | 'php-var-export'
  | 'php-array';

export type NativeFormatId = 'php-serialize' | 'php-var-export' | 'php-array';
export type ConversionDirection = 'parse' | 'stringify';

export type FormatDefinition = {
  readonly id: FormatId;
  readonly label: string;
  readonly canParse: boolean;
  readonly canStringify: boolean;
  readonly runtime: 'web' | 'native';
};

export type WebCodec = {
  readonly parse?: (text: string) => DataValue;
  readonly stringify?: (value: DataValue) => string;
};

export type NativeConversionRequest = {
  readonly format: NativeFormatId;
  readonly direction: ConversionDirection;
  readonly payload: string;
};

export type NativeExecutor = (request: NativeConversionRequest) => Promise<string>;

export type ConverterCapabilities = {
  readonly nativeFormats: readonly NativeFormatId[];
  readonly phpVersion: string | null;
};
