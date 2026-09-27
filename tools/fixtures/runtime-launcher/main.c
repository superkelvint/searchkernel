extern int searchkernel_runtime_fixture_answer(void);

int main(void) {
    return searchkernel_runtime_fixture_answer() == 42 ? 0 : 1;
}
