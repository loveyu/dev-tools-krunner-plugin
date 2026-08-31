export type LauncherToolId =
  'barcode' | 'convert' | 'image-compress' | 'image-editor' | 'json' | 'ocr' | 'watermark';

export type LauncherAction =
  | { readonly type: 'open-settings' }
  | { readonly type: 'open-tool'; readonly tool: LauncherToolId; readonly payload: string };

export type LauncherMatch = {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly keywords: readonly string[];
  readonly action: LauncherAction;
};

const JSON_TOOL: Omit<LauncherMatch, 'action'> = {
  id: 'json',
  title: 'ui.jsonWorkbench',
  description: 'ui.formatMinifySearchAndOpenDataConversion',
  keywords: ['json', 'j', 'JSON 工作台', 'JSON 工作臺'],
};

const TOOLS: readonly Omit<LauncherMatch, 'action'>[] = [
  JSON_TOOL,
  {
    id: 'convert',
    title: 'ui.dataConversion',
    description: 'ui.convertBetweenJsonYamlXmlUrlJwtAndMore',
    keywords: ['convert', 'cv', 'co', '数据转换', '資料轉換'],
  },
  {
    id: 'ocr',
    title: 'ui.ocrTextRecognition',
    description: 'ui.recognizeImageTextWithLocalTesseract',
    keywords: ['ocr', '文字识别', '文字辨識'],
  },
  {
    id: 'barcode',
    title: 'ui.barcodeAndQrCode',
    description: 'ui.recognizeAndGenerateBarcodesAndQrCodes',
    keywords: ['barcode', 'bar', 'qr', 'qrcode', '条码', '條碼', '二维码'],
  },
  {
    id: 'image-compress',
    title: 'ui.imageCompression',
    description: 'ui.frontendOnlyCompressionWithBeforeAndAfterComparison',
    keywords: ['compress', 'squoosh', 'image-compress', 'imgcompress', '图片压缩', '圖片壓縮'],
  },
  {
    id: 'image-editor',
    title: 'ui.imageEditor',
    description: 'ui.cropRotateDrawFilterCopyAndExport',
    keywords: [
      'editor',
      'image-editor',
      'edit-image',
      'imageedit',
      'imgedit',
      '图片编辑',
      '圖片編輯',
    ],
  },
  {
    id: 'watermark',
    title: 'watermark.title',
    description: 'watermark.launcherDescription',
    keywords: ['watermark', 'wm', 'image-watermark', 'watermarker', '图片水印', '圖片浮水印'],
  },
  {
    id: 'settings',
    title: 'ui.devtoolsSettings',
    description: 'ui.trayAutostartAppearanceLanguageAndGlobalShortcut',
    keywords: ['settings', 'setting', 'config', '设置', '設定'],
  },
];

export function matchLauncherQuery(query: string): readonly LauncherMatch[] {
  const trimmed = query.trim();
  const directJson = parseDirectJson(trimmed);
  if (directJson !== null) {
    return [toolMatch(JSON_TOOL, directJson)];
  }

  const [command = '', ...payloadParts] = trimmed.split(/\s+/u);
  const normalizedCommand = command.toLowerCase();
  const exact = TOOLS.find((tool) => tool.keywords.includes(normalizedCommand));
  if (exact !== undefined && trimmed !== '') {
    return [toolMatch(exact, payloadParts.join(' '))];
  }

  const candidates =
    normalizedCommand === ''
      ? TOOLS
      : TOOLS.filter(
          (tool) =>
            tool.title.toLowerCase().includes(normalizedCommand) ||
            tool.keywords.some(
              (keyword) =>
                keyword.startsWith(normalizedCommand) || normalizedCommand.startsWith(keyword),
            ),
        );
  return candidates.map((tool) => toolMatch(tool, ''));
}

function parseDirectJson(query: string): string | null {
  if (!(query.startsWith('{') || query.startsWith('['))) {
    return null;
  }
  try {
    JSON.parse(query);
    return query;
  } catch {
    return null;
  }
}

function toolMatch(tool: Omit<LauncherMatch, 'action'>, payload: string): LauncherMatch {
  const action: LauncherAction =
    tool.id === 'settings'
      ? { type: 'open-settings' }
      : {
          type: 'open-tool',
          tool: tool.id as LauncherToolId,
          payload,
        };
  return { ...tool, action };
}
