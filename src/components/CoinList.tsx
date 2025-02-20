import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { fromMojos } from '@/lib/utils';
import { t } from '@lingui/core/macro';
import { Trans } from '@lingui/react/macro';
import {
  ColumnDef,
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  RowSelectionState,
  SortingState,
  useReactTable,
} from '@tanstack/react-table';
import {
  ArrowDown,
  ArrowUp,
  ChevronLeft,
  ChevronRight,
  FilterIcon,
  FilterXIcon,
} from 'lucide-react';
import { useState } from 'react';
import { CoinRecord } from '../bindings';
import { Button } from './ui/button';
import { Checkbox } from './ui/checkbox';
import { NumberFormat } from './NumberFormat';

export interface CoinListProps {
  precision: number;
  coins: CoinRecord[];
  selectedCoins: RowSelectionState;
  setSelectedCoins: React.Dispatch<React.SetStateAction<RowSelectionState>>;
  actions?: React.ReactNode;
}

export default function CoinList(props: CoinListProps) {
  const [sorting, setSorting] = useState<SortingState>([
    { id: 'created_height', desc: true },
  ]);
  const [showUnspentOnly, setShowUnspentOnly] = useState(false);

  const columns: ColumnDef<CoinRecord>[] = [
    {
      id: 'select',
      header: ({ table }) => (
        <Checkbox
          className='mx-2'
          checked={
            table.getIsAllPageRowsSelected() ||
            (table.getIsSomePageRowsSelected() && 'indeterminate')
          }
          onCheckedChange={(value) => table.toggleAllPageRowsSelected(!!value)}
          aria-label={t`Select all coins`}
        />
      ),
      cell: ({ row }) => (
        <Checkbox
          className='mx-2'
          checked={row.getIsSelected()}
          onCheckedChange={(value) => row.toggleSelected(!!value)}
          aria-label={t`Select coin row`}
        />
      ),
      enableSorting: false,
      enableHiding: false,
    },
    {
      accessorKey: 'coin_id',
      header: ({ column }) => {
        return (
          <Button
            className='px-0'
            variant='link'
            onClick={() => column.toggleSorting(column.getIsSorted() === 'asc')}
          >
            <Trans>Coin</Trans>
            {column.getIsSorted() === 'asc' ? (
              <ArrowUp className='ml-2 h-4 w-4' aria-hidden='true' />
            ) : column.getIsSorted() === 'desc' ? (
              <ArrowDown className='ml-2 h-4 w-4' aria-hidden='true' />
            ) : (
              <span className='ml-2 w-4 h-4' />
            )}
          </Button>
        );
      },
      size: 100,
      cell: ({ row }) => (
        <div className='truncate overflow-hidden'>{row.original.coin_id}</div>
      ),
    },
    {
      accessorKey: 'amount',
      header: ({ column }) => {
        return (
          <Button
            className='px-0'
            variant='link'
            onClick={() => column.toggleSorting(column.getIsSorted() === 'asc')}
          >
            <Trans>Amount</Trans>
            {column.getIsSorted() === 'asc' ? (
              <ArrowUp className='ml-2 h-4 w-4' aria-hidden='true' />
            ) : column.getIsSorted() === 'desc' ? (
              <ArrowDown className='ml-2 h-4 w-4' aria-hidden='true' />
            ) : (
              <span className='ml-2 w-4 h-4' />
            )}
          </Button>
        );
      },
      cell: (info) => (
        <span className='font-mono'>
          <NumberFormat
            value={fromMojos(info.getValue() as string, props.precision)}
            minimumFractionDigits={0}
            maximumFractionDigits={props.precision}
          />
        </span>
      ),
    },
    {
      accessorKey: 'created_height',
      sortingFn: (rowA, rowB) => {
        const addSpend = 1_000_000_000;
        const addCreate = 2_000_000_000;

        const aPending =
          !!rowA.original.spend_transaction_id && !rowA.original.spent_height;
        const bPending =
          !!rowB.original.spend_transaction_id && !rowB.original.spent_height;

        const a =
          (rowA.original.created_height ?? 0) +
          (aPending
            ? addSpend
            : rowA.original.create_transaction_id
              ? addCreate
              : 0);

        const b =
          (rowB.original.created_height ?? 0) +
          (bPending
            ? addSpend
            : rowB.original.create_transaction_id
              ? addCreate
              : 0);

        return a < b ? -1 : a > b ? 1 : 0;
      },
      header: ({ column }) => {
        return (
          <Button
            className='px-0'
            variant='link'
            onClick={() => column.toggleSorting(column.getIsSorted() === 'asc')}
          >
            <Trans>Confirmed</Trans>
            {column.getIsSorted() === 'asc' ? (
              <ArrowUp className='ml-2 h-4 w-4' aria-hidden='true' />
            ) : column.getIsSorted() === 'desc' ? (
              <ArrowDown className='ml-2 h-4 w-4' aria-hidden='true' />
            ) : (
              <span className='ml-2 w-4 h-4' />
            )}
          </Button>
        );
      },
      cell: ({ row }) => (
        <div className='truncate overflow-hidden'>
          {row.original.created_height ??
            (row.original.create_transaction_id ? t`Pending...` : '')}
        </div>
      ),
    },
    {
      accessorKey: 'spent_height',
      sortingFn: (rowA, rowB) => {
        const a =
          (rowA.original.spent_height ?? 0) +
          (rowA.original.spend_transaction_id ? 10000000 : 0) +
          (rowA.original.offer_id ? 20000000 : 0);
        const b =
          (rowB.original.spent_height ?? 0) +
          (rowB.original.spend_transaction_id ? 10000000 : 0) +
          (rowB.original.offer_id ? 20000000 : 0);
        return a < b ? -1 : a > b ? 1 : 0;
      },
      header: ({ column }) => {
        return (
          <div className='flex items-center'>
            <Button
              className='px-0 mr-2'
              variant='link'
              onClick={() =>
                column.toggleSorting(column.getIsSorted() === 'asc')
              }
            >
              <Trans>Spent</Trans>
              {column.getIsSorted() === 'asc' ? (
                <ArrowUp className='ml-2 h-4 w-4' aria-hidden='true' />
              ) : column.getIsSorted() === 'desc' ? (
                <ArrowDown className='ml-2 h-4 w-4' aria-hidden='true' />
              ) : (
                <span className='ml-2 w-4 h-4' />
              )}
            </Button>
            <Button
              size='icon'
              variant='ghost'
              className='text-foreground'
              onClick={() => {
                setShowUnspentOnly(!showUnspentOnly);
                column.setFilterValue(showUnspentOnly ? t`Unspent` : '');

                if (!showUnspentOnly) {
                  setSorting([{ id: 'spent_height', desc: true }]);
                } else {
                  setSorting([{ id: 'created_height', desc: true }]);
                }
              }}
              aria-label={
                showUnspentOnly ? t`Show all coins` : t`Show unspent coins only`
              }
            >
              {showUnspentOnly ? (
                <FilterIcon className='h-4 w-4' aria-hidden='true' />
              ) : (
                <FilterXIcon className='h-4 w-4' aria-hidden='true' />
              )}
            </Button>
          </div>
        );
      },
      filterFn: (row, _, filterValue) => {
        return (
          filterValue === t`Unspent` &&
          !row.original.spend_transaction_id &&
          !row.original.spent_height &&
          !row.original.offer_id
        );
      },
      cell: ({ row }) => (
        <div className='truncate overflow-hidden'>
          {row.original.spent_height ??
            (row.original.spend_transaction_id
              ? t`Pending...`
              : row.original.offer_id
                ? t`Offered...`
                : '')}
        </div>
      ),
    },
  ];

  const table = useReactTable({
    data: props.coins,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    onSortingChange: setSorting,
    state: {
      sorting,
      rowSelection: props.selectedCoins,
    },
    getRowId: (row) => row.coin_id,
    onRowSelectionChange: props.setSelectedCoins,
    initialState: {
      pagination: {
        pageSize: 10,
      },
      columnFilters: [
        {
          id: 'spent_height',
          value: t`Unspent`,
        },
      ],
    },
  });

  return (
    <div>
      <div className='rounded-md border'>
        <Table>
          <TableHeader>
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow key={headerGroup.id}>
                {headerGroup.headers.map((header) => (
                  <TableHead key={header.id}>
                    {header.isPlaceholder
                      ? null
                      : flexRender(
                          header.column.columnDef.header,
                          header.getContext(),
                        )}
                  </TableHead>
                ))}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {table.getRowModel().rows?.length ? (
              table.getRowModel().rows.map((row) => (
                <TableRow
                  key={row.id}
                  data-state={row.getIsSelected() && 'selected'}
                  onClick={() => row.toggleSelected(!row.getIsSelected())}
                >
                  {row.getVisibleCells().map((cell) => (
                    <TableCell
                      key={cell.id}
                      style={{
                        maxWidth: cell.column.columnDef.size,
                      }}
                      className={
                        'h-12' +
                        ((row.original.spend_transaction_id &&
                          !row.original.spent_height) ||
                        row.original.create_transaction_id
                          ? ' pulsate-opacity'
                          : row.original.offer_id
                            ? ' pulsate-opacity'
                            : '')
                      }
                    >
                      {flexRender(
                        cell.column.columnDef.cell,
                        cell.getContext(),
                      )}
                    </TableCell>
                  ))}
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell
                  colSpan={columns.length}
                  className='h-24 text-center'
                >
                  <Trans>No results.</Trans>
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
      <div className='pt-4'>
        <div className='flex items-center justify-between'>
          <div className='flex space-x-2'>{props.actions}</div>
          <div className='flex space-x-2'>
            <Button
              variant='outline'
              size='icon'
              onClick={() => table.previousPage()}
              disabled={!table.getCanPreviousPage()}
              aria-label={t`Previous page`}
            >
              <ChevronLeft className='h-4 w-4' aria-hidden='true' />
            </Button>
            <Button
              variant='outline'
              size='icon'
              onClick={() => table.nextPage()}
              disabled={!table.getCanNextPage()}
              aria-label={t`Next page`}
            >
              <ChevronRight className='h-4 w-4' aria-hidden='true' />
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
