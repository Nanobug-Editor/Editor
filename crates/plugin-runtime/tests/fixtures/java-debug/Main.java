// Ordinary Java 8-compatible source used by both public Manager and native interaction acceptance.
import java.util.*;
public class Main {
 static volatile boolean ready;
 static class Node { int number = 42; Node next = this; }
 public static void main(String[] args) throws Exception {
  java.nio.file.Files.write(java.nio.file.Paths.get("starts.txt"), "start\n".getBytes("UTF-8"), java.nio.file.StandardOpenOption.CREATE, java.nio.file.StandardOpenOption.APPEND);
  Thread worker = new Thread(() -> hold(), "worker中文"); worker.start(); while (!ready) Thread.yield();
  Node node = new Node(); int[] numbers = new int[250]; for (int i=0;i<numbers.length;i++) numbers[i]=i;
  List<String> collection = Arrays.asList("first", "second", "third");
  int value = 7;
  int result = calculate(value); // BREAKPOINT
  System.out.println(value);
  String line = new Scanner(System.in, "UTF-8").nextLine();
  java.nio.file.Files.write(java.nio.file.Paths.get("debug-input.txt"), (line + "\n" + args[0]).getBytes("UTF-8"));
  worker.join();
 }
 static int calculate(int value) {
  int result = value + 1;
  return result;
 }
 static void hold() { int workerValue = 99; ready = true; while (true) { try { Thread.sleep(1000); } catch (InterruptedException error) { return; } } }
}
