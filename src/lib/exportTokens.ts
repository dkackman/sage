import { t } from '@lingui/core/macro';
import { toast } from 'react-toastify';
import { TokenRecord } from '@/types/TokenViewProps';
import { exportText } from '@/lib/exportText';

export async function exportTokens(tokens: TokenRecord[]) {
  try {
    if (tokens.length === 0) {
      toast.error(t`No tokens to export`);
      return;
    }

    // Create CSV content
    const headers = [
      'Name',
      'Ticker',
      'Asset ID',
      'Balance',
      'Balance (USD)',
      'Price (USD)',
      'Icon URL',
      'Type',
    ];

    const rows = tokens.map((token) => [
      (token.name || '').replace(/,/g, ''),
      (token.ticker || '').replace(/,/g, ''),
      token.asset_id || '',
      token.balance || '',
      token.balanceInUsd?.toString() || '',
      token.priceInUsd?.toString() || '',
      (token.icon_url || '').replace(/,/g, ''),
      token.isXch ? 'XCH' : 'CAT',
    ]);

    const csvContent = [
      headers.join(','),
      ...rows.map((row) => row.join(',')),
    ].join('\n');

    toast.dismiss();

    if (await exportText(csvContent, 'tokens')) {
      toast.success(t`Tokens exported successfully`);
    }
  } catch (error) {
    console.error('Failed to export tokens:', error);
    toast.dismiss();
    toast.error(t`Failed to export tokens: ${error}`);
  }
}
