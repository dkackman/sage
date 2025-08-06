import ConfirmationDialog from '@/components/ConfirmationDialog';
import { CreateProfileConfirmation } from '@/components/confirmations/CreateProfileConfirmation';
import Header from '@/components/Header';
import { Button } from '@/components/ui/button';
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/components/ui/form';
import { Input } from '@/components/ui/input';
import { FeeAmountInput } from '@/components/ui/masked-input';
import { useErrors } from '@/hooks/useErrors';
import { amount } from '@/lib/formTypes';
import { toMojos } from '@/lib/utils';
import { zodResolver } from '@hookform/resolvers/zod';
import { t } from '@lingui/core/macro';
import { Trans } from '@lingui/react/macro';
import { useState } from 'react';
import { useForm } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';
import * as z from 'zod';
import { commands, TransactionResponse } from '../bindings';
import Container from '../components/Container';
import { useWalletState } from '../state';

export default function CreateProfile() {
  const { addError } = useErrors();
  const navigate = useNavigate();
  const walletState = useWalletState();
  const [response, setResponse] = useState<TransactionResponse | null>(null);

  const formSchema = z.object({
    name: z.string().min(1, t`Name is required`),
    fee: amount(walletState.sync.unit.precision).optional(),
  });

  const form = useForm<z.infer<typeof formSchema>>({
    resolver: zodResolver(formSchema),
  });

  const onSubmit = (values: z.infer<typeof formSchema>) => {
    commands
      .createDid({
        name: values.name,
        fee: toMojos(
          values.fee?.toString() || '0',
          walletState.sync.unit.precision,
        ),
      })
      .then((data) => setResponse(data))
      .catch(addError);
  };

  return (
    <>
      <Header title={t`Create Profile`} />

      <Container className='max-w-xl'>
        <Form {...form}>
          <form onSubmit={form.handleSubmit(onSubmit)} className='space-y-4'>
            <FormField
              control={form.control}
              name='name'
              render={({ field }) => (
                <FormItem>
                  <FormLabel>
                    <Trans>Name</Trans>
                  </FormLabel>
                  <FormControl>
                    <Input placeholder={t`Display name`} {...field} />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />

            <div className='grid sm:grid-cols-2 gap-4'>
              <FormField
                control={form.control}
                name='fee'
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>
                      <Trans>Network Fee</Trans>
                    </FormLabel>
                    <FormControl>
                      <div className='relative'>
                        <FeeAmountInput {...field} className='pr-12' />
                      </div>
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
            </div>

            <Button type='submit'>
              <Trans>Create Profile</Trans>
            </Button>
          </form>
        </Form>
      </Container>

      <ConfirmationDialog
        response={response}
        close={() => setResponse(null)}
        onConfirm={() => navigate('/dids')}
        showRecipientDetails={false}
        additionalData={
          form.getValues().name
            ? {
                title: t`Profile Details`,
                content: (
                  <CreateProfileConfirmation name={form.getValues().name} />
                ),
              }
            : undefined
        }
      />
    </>
  );
}
